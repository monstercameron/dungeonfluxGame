#!/usr/bin/env python3
"""Meaningful exact hand-calculation checks for offline service model; no integration claims."""
import importlib.util,json
from pathlib import Path
spec=importlib.util.spec_from_file_location('economics',Path(__file__).with_name('service-economics.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
b=json.loads(Path(__file__).with_name('service-economics-inputs.json').read_text())['baseline'];zero={k:'0' for k in b};zero.update(price='100',active_campaigns='1',target_monthly_gross='100',subscription_charge_count='1',credit_reserve_fraction='1')
checks=[]
def check(name,inputs,expected):
 v=zero.copy();v.update(inputs);result=module.evaluate(v)
 for key,value in expected.items():assert module.dec(result[key])==module.dec(value),(name,key,result[key],value)
 checks.append({'name':name,'expected':expected,'pass':True})
check('subscription_tax_refund_fees',{'sales_tax_rate':'.1','refund_rate':'.1','payment_pct':'.03','payment_fixed':'1','income_tax_rate':'.2'}, {'recognized_subscription_gross':'100','net_subscription_revenue':'90','sales_tax_collected':'10','sales_tax_refunded':'1','sales_tax_net_remitted':'9','processing_and_billing_fees':'4.3','consolidated_operating_pre_tax':'85.7','illustrative_income_tax':'17.14','cash_change_after_tax_remittance':'68.56'})
check('unused_credit_is_cash_not_profit',{'credit_topups':'100','credit_transactions':'1','payment_pct':'.03','payment_fixed':'1','opening_cash':'1000'}, {'credit_contribution':'-4','ending_deferred_credit_liability':'100','consolidated_operating_pre_tax':'92','cash_change_after_tax_remittance':'192','ending_liquid_cash':'1192','ending_available_cash_after_unknown_hold':'1092'})
check('opening_credit_consumption_refund_and_tax',{'credit_opening_balance':'100','credit_consumed':'40','credit_refunds':'10','credit_supplier_cost':'25','income_tax_rate':'.2','unknown_liability_added':'20','opening_cash':'1000'}, {'credit_contribution':'15','ending_deferred_credit_liability':'50','consolidated_operating_pre_tax':'115','illustrative_income_tax':'23','cash_change_after_tax_remittance':'42','ending_liquid_cash':'1042','ending_available_cash_after_unknown_hold':'972'})
check('dispute_principal_and_fees',{'dispute_rate':'.1','dispute_fee':'30'}, {'chargeback_principal_loss':'10','net_subscription_revenue':'90','dispute_cost':'3','consolidated_operating_pre_tax':'87','cash_change_after_tax_remittance':'87'})
check('tax_inclusive_receipt',{'price':'110','sales_tax_rate':'.1','tax_inclusive':'1'}, {'recognized_subscription_gross':'100','sales_tax_collected':'10','cash_collected_including_tax_topups':'110','cash_change_after_tax_remittance':'100'})
for key,value in [('price','NaN'),('raw_session_cost','Infinity'),('tax_inclusive','2'),('active_campaigns','1.5'),('subscription_charge_count','1.1'),('credit_consumed','1')]:
 v=zero.copy();v[key]=value
 try:module.evaluate(v)
 except ValueError:checks.append({'name':'reject_'+key+'_'+value,'pass':True})
 else:raise AssertionError((key,value))
p=Path(__file__).parent/'evidence/service-economics-checks.json';p.write_text(json.dumps({'scope':'Exact offline model arithmetic checks only','checks':checks},indent=2)+'\n');print(json.dumps({'passed':len(checks),'evidence':str(p)}))
