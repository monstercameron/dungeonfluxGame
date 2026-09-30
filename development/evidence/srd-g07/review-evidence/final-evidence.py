import json, pathlib, hashlib, subprocess, time, datetime
from pypdf import PdfReader
r=pathlib.Path('/Users/earlcameron/Desktop/dungeonflux')
e=r/'development/evidence/srd-g07/review-evidence'
w=r/'artifacts/worktrees/pin-srd-g07'
t=r/'artifacts/tmp/PIN-G07-SRD-001-a1-review'
url='https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf'
cmd=['curl','--fail','--location','--proto','=https','--max-time','55','--max-filesize','67108864','--silent','--show-error','--dump-header',str(e/'publisher-download.headers'),'--output',str(t/'publisher-check.pdf'),url]
s=time.monotonic(); p=subprocess.run(cmd,capture_output=True,text=True,timeout=58)
out={'timestamp_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'command':cmd,'exit':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'seconds':time.monotonic()-s,'publisher_page':'https://www.dndbeyond.com/srd','license_source':'https://creativecommons.org/licenses/by/4.0/legalcode','creator_faq':'https://www.dndbeyond.com/creator-faq'}
if p.returncode==0:
 b=(t/'publisher-check.pdf').read_bytes();out.update(bytes=len(b),sha256=hashlib.sha256(b).hexdigest(),identical_to_pin=b==(w/'development/rules-sources/srd-5.2.1-en.pdf').read_bytes())
(e/'publisher-verification.json').write_text(json.dumps(out,indent=2)+'\n')
m=json.loads((w/'development/rules-sources/manifest.json').read_text());reader=PdfReader(str(w/'development/rules-sources/srd-5.2.1-en.pdf'),strict=True)
loc=[]
for item in m['verified_locators']:
 text=reader.pages[item['pdf_page']-1].extract_text() or ''
 loc.append({'locator':item,'page_text_prefix':text[:650],'label_found':item['label'].lower() in text.lower()})
visual={'page_count':len(reader.pages),'rendered_and_vision_inspected_pages':[1,2,3,4],'method':'Bundled Poppler pdftoppm at maximum 1500 pixels; each PNG opened with view_image and visually inspected. No PDF changes.','observations':{'1':'Legal Information page clearly identifies System Reference Document 5.2.1, Wizards of the Coast LLC, CC-BY-4.0 and complete required attribution. Footer page 1.','2':'Contents shows all section-start page numbers retained by manifest; legible beginning of monster index. Footer page 2.','3':'Index continues alphabetically from Ankheg through Phase Spider, consistent with A-P locator. Footer page 3.','4':'Index continues Piranha through Zombie, consistent with P-Z locator. Footer page 4.'},'section_locators':loc,'full_book_gate':'SRD subset only; no required PHB/DMG/MM bytes, revisions, errata/catalog denominator or reviewed production RightsGrant.'}
(e/'pdf-observations.json').write_text(json.dumps(visual,indent=2)+'\n')
print(json.dumps(out,indent=2));print('page_count',len(reader.pages))
