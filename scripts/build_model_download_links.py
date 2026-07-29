import getpass
import json
import logging
from pathlib import Path

from boxsdk.auth.oauth2 import OAuth2
from boxsdk.client.client import Client

log = logging.getLogger()
log.setLevel(logging.INFO)

# these all come from the box developer app
CLIENT_ID = input("Client ID: ")
CLIENT_SECRET = getpass.getpass("Client Secret: ")
ACCESS_TOKEN = getpass.getpass("Access Token: ")

# pull this from the box url
FOLDER_ID = input("Box Folder Id: ")

SRC_DIR = Path(__file__).parents[1]
OUTDIR = SRC_DIR / Path("python/nrel/routee/compass/resources/models")

oauth2 = OAuth2(CLIENT_ID, CLIENT_SECRET, access_token=ACCESS_TOKEN)
client = Client(oauth2)

folder = client.folder(folder_id=FOLDER_ID)

files = folder.get_items()

download_links = {}
for f in files:
    name = f.name.split(".")[0]
    log.info(f"working on {name}")
    download_links[name] = f.get_shared_link_download_url(access="open")


log.info(f"writing links to {OUTDIR}")
outfile = OUTDIR / Path("download_links.json")
with open(outfile, "w", encoding="utf-8") as f:
    json.dump(download_links, f, ensure_ascii=False, indent=4)
