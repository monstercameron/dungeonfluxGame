"""Independent transaction oracles against explicitly frozen offline planning model."""
from pathlib import Path
from decimal import Decimal
import argparse,hashlib,importlib.util,json,sys
sys.dont_write_bytecode=True
p=argparse.ArgumentParser();p.add_argument('--script-sha',required=True);p.add_argument('--inputs-sha',required=True);p.add_argument('--output',required=True);a=p.parse_args()
root=Path(__file__).resolve().parents[3];source=root/'service-economics.py';inputs=root/'service-economics-inputs.json'
assert hashlib.sha256(source.read_bytes()).hexdigest()==a.script_sha,'Source changed since freeze'
assert hashlib.sha256(inputs.read_bytes()).hexdigest()==a.inputs_sha,'Inputs changed since freeze'
spec=importlib.util.spec_from_file_location('frozen_economics',source);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
base=json.loads(inputs.read_text())['baseline'];fixture=json.loads(Path(__file__).with_name('oracles.json').read_text())
# This explicit map binds semantic independently derived transaction expectations to named outputs.
fields={'subscription_net_revenue':'net_subscription_revenue','consolidated_operating_pre_tax':'consolidated_operating_pre_tax','income_tax':'illustrative_income_tax','bank_cash_change':'cash_change_after_tax_remittance','closing_credit_liability':'ending_deferred_credit_liability','sales_tax_payable':'sales_tax_net_remitted','unknown_risk_held':'unknown_liability_held','available_cash_increment_after_unknown':'ending_available_cash_after_unknown_hold'}
def neutral():
 row={k:'0' for k in base};row.update(price='100',active_campaigns='1',subscription_charge_count='1',billed_attempt_multiplier='1');return row
results=[]
for case in fixture['cases']:
 row=neutral();row.update(case['overrides']);out=module.evaluate(row)
 checks={k:{'expected':v,'actual':out[fields[k]],'pass':Decimal(v)==Decimal(out[fields[k]])} for k,v in case['expected'].items()}
 results.append({'id':case['id'],'inputs':row,'transaction_oracle':case['transactions'],'checks':checks,'pass':all(v['pass'] for v in checks.values())})
# Five USD100 successful charges; one full USD100 chargeback and USD50 other refund.
# Tax10% => collected550, refunded165 inclusive tax, remitted35. Original fees18;
# dispute fee5. Net350, OP327, assumed tax65.40, bankchange261.60.
row=neutral();row.update(active_campaigns='5',subscription_charge_count='5',refund_rate='.10',dispute_rate='.20',dispute_fee='5',sales_tax_rate='.10',income_tax_rate='.20',payment_pct='.03',payment_fixed='.30')
out=module.evaluate(row);expected={'net_subscription_revenue':'350','chargeback_principal_loss':'100','sales_tax_net_remitted':'35','consolidated_operating_pre_tax':'327','illustrative_income_tax':'65.40','cash_change_after_tax_remittance':'261.60'}
checks={k:{'expected':v,'actual':out[k],'pass':Decimal(v)==Decimal(out[k])} for k,v in expected.items()};results.append({'id':'CHARGEBACK_PRINCIPAL_AND_ORIGINAL_FEES','inputs':row,'checks':checks,'pass':all(v['pass'] for v in checks.values())})
negatives=[]
for probe in fixture['negative_inputs']+[{'field':'subscription_charge_count','value':'1.5'},{'field':'credit_reserve_fraction','value':'1.1'},{'field':'new_retained_campaigns','value':'0.5'}]:
 row=neutral();row[probe['field']]=probe['value']
 try:module.evaluate(row);rejected=False;error=None
 except (ValueError,ArithmeticError) as e:rejected=True;error=type(e).__name__+': '+str(e)
 negatives.append(dict(probe,rejected=rejected,error=error))
assert hashlib.sha256(source.read_bytes()).hexdigest()==a.script_sha,'Source changed during checks'
assert hashlib.sha256(inputs.read_bytes()).hexdigest()==a.inputs_sha,'Inputs changed during checks'
data={'source_sha256':a.script_sha,'inputs_sha256':a.inputs_sha,'oracle_sha256':hashlib.sha256(Path(__file__).with_name('oracles.json').read_bytes()).hexdigest(),'semantic_field_map':fields,'scope':'Executed actual frozen offline model; exact independent toy transaction oracles and negative input rejection, not real supplier/customer/ledger runtime acceptance','positive_cases':results,'negative_probes':negatives,'pass':all(r['pass'] for r in results) and all(r['rejected'] for r in negatives)}
Path(a.output).write_text(json.dumps(data,indent=2)+'\n');print(json.dumps({'pass':data['pass'],'positive_cases':len(results),'negative_probes':len(negatives),'source_sha256':a.script_sha}));sys.exit(0 if data['pass'] else 1)
