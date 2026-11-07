#!/usr/bin/env python3
"""Read-only installed launcher and logo audit. Never removes user launchers."""
import configparser
import os
from pathlib import Path
roots=[Path(os.environ.get('XDG_DATA_HOME',Path.home()/'.local/share'))]
roots += [Path(p) for p in os.environ.get('XDG_DATA_DIRS','/usr/local/share:/usr/share').split(':') if p]
roots += [Path(os.environ.get('XDG_CONFIG_HOME',Path.home()/'.config'))]
found=[]
for root in roots:
    for folder in ['applications','autostart']:
        for path in (root/folder).glob('*.desktop'):
            try:
                parser=configparser.ConfigParser(interpolation=None,strict=False)
                parser.read(path)
                entry=parser['Desktop Entry']
                if 'clipvault' not in (entry.get('Name','')+' '+entry.get('Exec','')).lower(): continue
                found.append(path)
                print(f'{path}\n  Name={entry.get("Name")}\n  Exec={entry.get("Exec")}\n  Icon={entry.get("Icon")}\n  StartupWMClass={entry.get("StartupWMClass")}')
                icon=entry.get('Icon','')
                if icon.startswith('/'):
                    print('  Icon file exists:',Path(icon).is_file())
                else:
                    paths=[p for r in roots for p in (r/'icons').glob(f'*/**/apps/{icon}.*')]
                    print('  Installed theme icons:',*[str(p) for p in paths],sep='\n    ')
            except (OSError,configparser.Error,KeyError) as error:
                print(path,error)
print(f'{len(found)} related launchers found. Autostart and application launchers serve different purposes.')
print('Inspect duplicate application entries and their Exec targets manually. No entries were removed.')
print('On X11: run xprop WM_CLASS _NET_WM_ICON, then click the ClipVault window; expected WM_CLASS contains clipvault.')
