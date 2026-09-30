#!/usr/bin/env python3
"""Offline planning sensitivity only; no customer/provider/payment calls."""
import argparse, copy, hashlib, json
from decimal import Decimal, ROUND_CEILING
from pathlib import Path
D = Decimal

def dec(value):
    return D(str(value))

def evaluate(values):
    p = {k: dec(v) for k, v in values.items()}
    for key, value in p.items():
        if not value.is_finite() or value < 0:
            raise ValueError(f'Negative assumption: {key}')
    for key in ('refund_rate','bad_debt_rate','monthly_churn','payment_pct','sales_tax_rate','income_tax_rate','dispute_rate'):
        if p[key] >= 1:
            raise ValueError(f'Invalid fraction: {key}')
    if p['credit_reserve_fraction'] > 1:
        raise ValueError('Invalid credit reserve fraction')
    if p['tax_inclusive'] not in (D(0),D(1)):
        raise ValueError('tax_inclusive must be 0/1')
    for key in ('active_campaigns','new_retained_campaigns','credit_transactions','subscription_charge_count'):
        if p[key] != p[key].to_integral_value():
            raise ValueError(f'Nonintegral count: {key}')
    if p['refund_rate'] + p['bad_debt_rate'] + p['dispute_rate'] >= 1 or p['price'] <= 0 or p['active_campaigns'] <= 0:
        raise ValueError('Invalid revenue base')
    n = p['active_campaigns']
    gross = n * p['price'] / (1 + p['sales_tax_rate'] * p['tax_inclusive'])
    refunded = gross * p['refund_rate']
    bad_debt = gross * p['bad_debt_rate']
    chargeback_principal = gross * p['dispute_rate']
    net = gross - refunded - bad_debt - chargeback_principal
    taxable_receipts = gross - bad_debt
    tax_collected = taxable_receipts * p['sales_tax_rate']
    tax_refunded = (refunded + chargeback_principal) * p['sales_tax_rate']
    tax_payable = tax_collected - tax_refunded
    fees = (taxable_receipts + tax_collected) * p['payment_pct'] + p['subscription_charge_count'] * p['payment_fixed']
    disputes = n * p['dispute_rate'] * p['dispute_fee']
    usage = n * p['sessions'] * p['raw_session_cost'] * p['billed_attempt_multiplier']
    support = n * p['support_minutes'] / 60 * p['support_hourly']
    delivery_rights = n * (p['delivery_per_campaign'] + p['rights_per_campaign'])
    contribution = net - fees - disputes - usage - support - delivery_rights
    replacements = n * p['monthly_churn']
    acquisition = (replacements + p['new_retained_campaigns']) * p['cac']
    operating = contribution - acquisition - p['fixed_operations']
    # Consolidated income tax is computed after earned credit margin below.
    # Top-up is cash, consumption is recognized revenue. No free earnings on unused credit.
    earned_credit = p['credit_consumed']
    credit_fees = p['credit_topups'] * p['payment_pct'] + p['credit_transactions'] * p['payment_fixed']
    credit_contribution = earned_credit - p['credit_supplier_cost'] - credit_fees
    deferred_delta = p['credit_topups'] - earned_credit - p['credit_refunds']
    if earned_credit + p['credit_refunds'] > p['credit_opening_balance'] + p['credit_topups']:
        raise ValueError('Credit consumption/refund exceeds liability')
    consolidated = operating + credit_contribution
    income_tax = max(consolidated, D(0)) * p['income_tax_rate']
    # Illustrative full sales-tax reversal on refunded or disputed subscriptions. Credit tax jurisdiction remains separate assumed zero.
    cash_collected = taxable_receipts + tax_collected + p['credit_topups']
    cash_refunds = refunded + tax_refunded + chargeback_principal + p['credit_refunds']
    cash_costs = fees + credit_fees + disputes + usage + support + delivery_rights + acquisition + p['fixed_operations'] + p['credit_supplier_cost'] + income_tax
    cash = cash_collected - cash_refunds - tax_payable - cash_costs
    liquid = p['opening_cash'] + cash
    closing_credit = p['credit_opening_balance'] + deferred_delta
    credit_reserve = closing_credit * p['credit_reserve_fraction']
    available = liquid - p['unknown_liability_added'] - credit_reserve
    per_contribution = contribution / n
    per_after_replacement = per_contribution - p['monthly_churn'] * p['cac']
    return {k: str(v) if isinstance(v,D) else v for k,v in {
        'recognized_subscription_gross':gross, 'net_subscription_revenue':net,'chargeback_principal_loss':chargeback_principal,'sales_tax_collected':tax_collected,'sales_tax_refunded':tax_refunded,'sales_tax_net_remitted':tax_payable,
        'processing_and_billing_fees':fees,'dispute_cost':disputes,'generation_cogs':usage,'support_labor':support,'delivery_and_rights':delivery_rights,
        'subscription_contribution':contribution,'contribution_margin':contribution/net if net else None,
        'replacement_campaigns_month':replacements,'acquisition_cost':acquisition,'operating_pre_tax_excluding_credit':operating,
        'consolidated_operating_pre_tax':consolidated,'illustrative_income_tax':income_tax,'earned_credit_revenue':earned_credit,'credit_contribution':credit_contribution,
        'deferred_credit_liability_delta':deferred_delta,'unknown_liability_held':p['unknown_liability_added'],
        'cash_collected_including_tax_topups':cash_collected,'cash_refunds_including_tax':cash_refunds,'cash_operating_payments':cash_costs,'cash_change_after_tax_remittance':cash,'ending_liquid_cash':liquid,'ending_available_cash_after_unknown_hold':available,'opening_available_cash_after_credit_reserve':p['opening_cash']-p['credit_opening_balance']*p['credit_reserve_fraction'],'available_cash_change':available-(p['opening_cash']-p['credit_opening_balance']*p['credit_reserve_fraction']),'available_cash_shortfall':max(-available,D(0)),'ending_deferred_credit_liability':closing_credit,'credit_liability_cash_reserve':credit_reserve,
        'campaigns_for_target_gross':(p['target_monthly_gross']/ (p['price']/(1+p['sales_tax_rate']*p['tax_inclusive']))).to_integral_value(rounding=ROUND_CEILING),
        'steady_state_subscription_only_break_even_campaigns_excludes_growth_credit':(p['fixed_operations']/per_after_replacement).to_integral_value(rounding=ROUND_CEILING) if per_after_replacement>0 else None,
        'cac_payback_months':p['cac']/per_contribution if per_contribution>0 else None,
        'month_12_retained_fraction_no_new_acquisition':(1-p['monthly_churn'])**12,
        'bank_only_stationary_cash_burn_months_not_reserve_runway':p['opening_cash']/(-cash) if cash<0 else None
    }.items()}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs',type=Path,default=Path(__file__).with_name('service-economics-inputs.json'))
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args(); raw=args.inputs.read_bytes(); data=json.loads(raw)
    output={'scope':'Offline assumed scenarios, not observed profit or demand','inputs_sha256':hashlib.sha256(raw).hexdigest(),'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scenarios':{}}
    for name,overrides in data['scenarios'].items():
        row=copy.deepcopy(data['baseline']);row.update(overrides);output['scenarios'][name]={'inputs':row,'results':evaluate(row)}
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(output,indent=2)+'\n')
if __name__=='__main__':main()
