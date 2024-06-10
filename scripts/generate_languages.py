#!/usr/bin/env python3
import urllib.request
import json

# Scipt used to generate entries for LANGUAGES static.

def main():
    response = urllib.request.urlopen("https://api.gog.com/v1/languages")
    data = response.read()
    languages = json.loads(data)

    for lang in languages["_embedded"]["items"]:
        code = lang["code"]
        name = lang["name"]
        native_name = lang["nativeName"]
        deprecated_codes = [f"\"{n}\"" for n in lang["deprecatedCodes"]]
        print("Language {" + 'name: "{}", code: "{}", native_name: "{}", deprecated_codes: &[{}]'.format(
            name, code, native_name, ",".join(deprecated_codes)) + "},")


if __name__ == "__main__":
    main()
