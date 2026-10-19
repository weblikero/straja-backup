"""Build embedded locale catalog from the reviewed UTF-8 translation table."""
import json,re
from pathlib import Path
root=Path(__file__).resolve().parent.parent
rows=[]
seen=set()
for line in (root/'locales/messages.txt').read_text(encoding='utf-8-sig').splitlines():
 if not line or line.startswith('#'): continue
 cols=[v.replace(r'\n','\n').replace('\\\\','\\') for v in line.split('|')]
 if len(cols)!=7: raise ValueError(f'Expected 7 columns: {line[:100]}')
 if cols[0] in seen: raise ValueError(f'Duplicate: {cols[0]}')
 seen.add(cols[0])
 source=cols[0]
 slots=re.findall(r'\{[^{}]*\}',source)
 parts=re.split(r'\{[^{}]*\}',source)
 if slots:
  for value in cols[1:]:
   if sorted(re.findall(r'\{(\d+)\}',value))!=sorted(str(i) for i in range(len(slots))):
    raise ValueError(f'Placeholder mismatch: {source} -> {value}')
 rows.append({'source':source,'parts':parts,'values':cols[1:]})
(root/'locales/catalog.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(f'{len(rows)} messages, 7 languages; placeholders checked.')
