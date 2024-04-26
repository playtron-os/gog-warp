use lazy_static::lazy_static;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone)]
pub struct Language<'a> {
    pub code: &'a str,
    pub name: &'a str,
    pub native_name: &'a str,
    pub deprecated_codes: Vec<&'a str>,
}

lazy_static! {
    pub static ref LANGUAGES: [Language<'static>; 84] = [
        Language {
            name: "Afrikaans",
            code: "af-ZA",
            native_name: "Afrikaans",
            deprecated_codes: vec![]
        },
        Language {
            name: "Arabic",
            code: "ar",
            native_name: "العربية",
            deprecated_codes: vec![]
        },
        Language {
            name: "Azeri",
            code: "az-AZ",
            native_name: "Azərbaycan­ılı",
            deprecated_codes: vec![]
        },
        Language {
            name: "Belarusian",
            code: "be-BY",
            native_name: "Беларускі",
            deprecated_codes: vec!["be"]
        },
        Language {
            name: "Bengali",
            code: "bn-BD",
            native_name: "বাংলা",
            deprecated_codes: vec!["bn_BD"]
        },
        Language {
            name: "Bulgarian",
            code: "bg-BG",
            native_name: "български",
            deprecated_codes: vec!["bg", "bl"]
        },
        Language {
            name: "Bosnian",
            code: "bs-BA",
            native_name: "босански",
            deprecated_codes: vec![]
        },
        Language {
            name: "Catalan",
            code: "ca-ES",
            native_name: "Català",
            deprecated_codes: vec!["ca"]
        },
        Language {
            name: "Czech",
            code: "cs-CZ",
            native_name: "Čeština",
            deprecated_codes: vec!["cz"]
        },
        Language {
            name: "Welsh",
            code: "cy-GB",
            native_name: "Cymraeg",
            deprecated_codes: vec![]
        },
        Language {
            name: "Danish",
            code: "da-DK",
            native_name: "Dansk",
            deprecated_codes: vec!["da"]
        },
        Language {
            name: "German",
            code: "de-DE",
            native_name: "Deutsch",
            deprecated_codes: vec!["de"]
        },
        Language {
            name: "Divehi",
            code: "dv-MV",
            native_name: "ދިވެހިބަސް",
            deprecated_codes: vec![]
        },
        Language {
            name: "Greek",
            code: "el-GR",
            native_name: "ελληνικά",
            deprecated_codes: vec!["gk", "el-GK"]
        },
        Language {
            name: "British English",
            code: "en-GB",
            native_name: "British English",
            deprecated_codes: vec!["en_GB"]
        },
        Language {
            name: "English",
            code: "en-US",
            native_name: "English",
            deprecated_codes: vec!["en"]
        },
        Language {
            name: "Spanish",
            code: "es-ES",
            native_name: "Español",
            deprecated_codes: vec!["es"]
        },
        Language {
            name: "Latin American Spanish",
            code: "es-MX",
            native_name: "Español (AL)",
            deprecated_codes: vec!["es_mx"]
        },
        Language {
            name: "Estonian",
            code: "et-EE",
            native_name: "Eesti",
            deprecated_codes: vec!["et"]
        },
        Language {
            name: "Basque",
            code: "eu-ES",
            native_name: "Euskara",
            deprecated_codes: vec![]
        },
        Language {
            name: "Persian",
            code: "fa-IR",
            native_name: "فارسى",
            deprecated_codes: vec!["fa"]
        },
        Language {
            name: "Finnish",
            code: "fi-FI",
            native_name: "Suomi",
            deprecated_codes: vec!["fi"]
        },
        Language {
            name: "Faroese",
            code: "fo-FO",
            native_name: "Føroyskt",
            deprecated_codes: vec![]
        },
        Language {
            name: "French",
            code: "fr-FR",
            native_name: "Français",
            deprecated_codes: vec!["fr"]
        },
        Language {
            name: "Galician",
            code: "gl-ES",
            native_name: "Galego",
            deprecated_codes: vec![]
        },
        Language {
            name: "Gujarati",
            code: "gu-IN",
            native_name: "ગુજરાતી",
            deprecated_codes: vec!["gu"]
        },
        Language {
            name: "Hebrew",
            code: "he-IL",
            native_name: "עברית",
            deprecated_codes: vec!["he"]
        },
        Language {
            name: "Hindi",
            code: "hi-IN",
            native_name: "हिंदी",
            deprecated_codes: vec!["hi"]
        },
        Language {
            name: "Croatian",
            code: "hr-HR",
            native_name: "Hrvatski",
            deprecated_codes: vec![]
        },
        Language {
            name: "Hungarian",
            code: "hu-HU",
            native_name: "Magyar",
            deprecated_codes: vec!["hu"]
        },
        Language {
            name: "Armenian",
            code: "hy-AM",
            native_name: "Հայերեն",
            deprecated_codes: vec![]
        },
        Language {
            name: "Indonesian",
            code: "id-ID",
            native_name: "Bahasa Indonesia",
            deprecated_codes: vec![]
        },
        Language {
            name: "Icelandic",
            code: "is-IS",
            native_name: "Íslenska",
            deprecated_codes: vec!["is"]
        },
        Language {
            name: "Italian",
            code: "it-IT",
            native_name: "Italiano",
            deprecated_codes: vec!["it"]
        },
        Language {
            name: "Japanese",
            code: "ja-JP",
            native_name: "日本語",
            deprecated_codes: vec!["jp"]
        },
        Language {
            name: "Javanese",
            code: "jv-ID",
            native_name: "ꦧꦱꦗꦮ",
            deprecated_codes: vec!["jv"]
        },
        Language {
            name: "Georgian",
            code: "ka-GE",
            native_name: "ქართული",
            deprecated_codes: vec![]
        },
        Language {
            name: "Kazakh",
            code: "kk-KZ",
            native_name: "Қазақ",
            deprecated_codes: vec![]
        },
        Language {
            name: "Kannada",
            code: "kn-IN",
            native_name: "ಕನ್ನಡ",
            deprecated_codes: vec![]
        },
        Language {
            name: "Korean",
            code: "ko-KR",
            native_name: "한국어",
            deprecated_codes: vec!["ko"]
        },
        Language {
            name: "Konkani",
            code: "kok-IN",
            native_name: "कोंकणी",
            deprecated_codes: vec![]
        },
        Language {
            name: "Kyrgyz",
            code: "ky-KG",
            native_name: "Кыргыз",
            deprecated_codes: vec![]
        },
        Language {
            name: "Latin",
            code: "la",
            native_name: "latine",
            deprecated_codes: vec![]
        },
        Language {
            name: "Lithuanian",
            code: "lt-LT",
            native_name: "Lietuvių",
            deprecated_codes: vec![]
        },
        Language {
            name: "Latvian",
            code: "lv-LV",
            native_name: "Latviešu",
            deprecated_codes: vec![]
        },
        Language {
            name: "Malayalam",
            code: "ml-IN",
            native_name: "മലയാളം",
            deprecated_codes: vec!["ml"]
        },
        Language {
            name: "Maori",
            code: "mi-NZ",
            native_name: "Reo Māori",
            deprecated_codes: vec![]
        },
        Language {
            name: "Macedonian",
            code: "mk-MK",
            native_name: "Mакедонски јазик",
            deprecated_codes: vec![]
        },
        Language {
            name: "Mongolian",
            code: "mn-MN",
            native_name: "Монгол хэл",
            deprecated_codes: vec![]
        },
        Language {
            name: "Marathi",
            code: "mr-IN",
            native_name: "मराठी",
            deprecated_codes: vec!["mr"]
        },
        Language {
            name: "Malay",
            code: "ms-MY",
            native_name: "Bahasa Malaysia",
            deprecated_codes: vec![]
        },
        Language {
            name: "Maltese",
            code: "mt-MT",
            native_name: "Malti",
            deprecated_codes: vec![]
        },
        Language {
            name: "Norwegian",
            code: "nb-NO",
            native_name: "Norsk",
            deprecated_codes: vec!["no"]
        },
        Language {
            name: "Dutch",
            code: "nl-NL",
            native_name: "Nederlands",
            deprecated_codes: vec!["nl"]
        },
        Language {
            name: "Northern Sotho",
            code: "ns-ZA",
            native_name: "Sesotho sa Leboa",
            deprecated_codes: vec![]
        },
        Language {
            name: "Punjabi",
            code: "pa-IN",
            native_name: "ਪੰਜਾਬੀ",
            deprecated_codes: vec![]
        },
        Language {
            name: "Polish",
            code: "pl-PL",
            native_name: "Polski",
            deprecated_codes: vec!["pl"]
        },
        Language {
            name: "Pashto",
            code: "ps-AR",
            native_name: "پښتو",
            deprecated_codes: vec![]
        },
        Language {
            name: "Portuguese (Brazilian)",
            code: "pt-BR",
            native_name: "Português do Brasil",
            deprecated_codes: vec!["br"]
        },
        Language {
            name: "Portuguese",
            code: "pt-PT",
            native_name: "Português",
            deprecated_codes: vec!["pt"]
        },
        Language {
            name: "Romanian",
            code: "ro-RO",
            native_name: "Română",
            deprecated_codes: vec!["ro"]
        },
        Language {
            name: "Russian",
            code: "ru-RU",
            native_name: "Pусский",
            deprecated_codes: vec!["ru"]
        },
        Language {
            name: "Sanskrit",
            code: "sa-IN",
            native_name: "संस्कृत",
            deprecated_codes: vec![]
        },
        Language {
            name: "Slovak",
            code: "sk-SK",
            native_name: "Slovenčina",
            deprecated_codes: vec!["sk"]
        },
        Language {
            name: "Slovenian",
            code: "sl-SI",
            native_name: "Slovenski",
            deprecated_codes: vec![]
        },
        Language {
            name: "Albanian",
            code: "sq-AL",
            native_name: "Shqipe",
            deprecated_codes: vec![]
        },
        Language {
            name: "Serbian",
            code: "sr-SP",
            native_name: "Srpski",
            deprecated_codes: vec!["sb"]
        },
        Language {
            name: "Swedish",
            code: "sv-SE",
            native_name: "Svenska",
            deprecated_codes: vec!["sv"]
        },
        Language {
            name: "Kiswahili",
            code: "sw-KE",
            native_name: "Kiswahili",
            deprecated_codes: vec![]
        },
        Language {
            name: "Tamil",
            code: "ta-IN",
            native_name: "தமிழ்",
            deprecated_codes: vec!["ta_IN"]
        },
        Language {
            name: "Telugu",
            code: "te-IN",
            native_name: "తెలుగు",
            deprecated_codes: vec!["te"]
        },
        Language {
            name: "Thai",
            code: "th-TH",
            native_name: "ไทย",
            deprecated_codes: vec!["th"]
        },
        Language {
            name: "Tagalog",
            code: "tl-PH",
            native_name: "Filipino",
            deprecated_codes: vec![]
        },
        Language {
            name: "Setswana",
            code: "tn-ZA",
            native_name: "Setswana",
            deprecated_codes: vec![]
        },
        Language {
            name: "Turkish",
            code: "tr-TR",
            native_name: "Türkçe",
            deprecated_codes: vec!["tr"]
        },
        Language {
            name: "Tatar",
            code: "tt-RU",
            native_name: "Татар",
            deprecated_codes: vec![]
        },
        Language {
            name: "Ukrainian",
            code: "uk-UA",
            native_name: "Українська",
            deprecated_codes: vec!["uk"]
        },
        Language {
            name: "Urdu",
            code: "ur-PK",
            native_name: "اُردو",
            deprecated_codes: vec!["ur_PK"]
        },
        Language {
            name: "Uzbek",
            code: "uz-UZ",
            native_name: "U'zbek",
            deprecated_codes: vec![]
        },
        Language {
            name: "Vietnamese",
            code: "vi-VN",
            native_name: "Tiếng Việt",
            deprecated_codes: vec!["vi"]
        },
        Language {
            name: "isiXhosa",
            code: "xh-ZA",
            native_name: "isiXhosa",
            deprecated_codes: vec![]
        },
        Language {
            name: "Chinese (Simplified)",
            code: "zh-Hans",
            native_name: "中文(简体)",
            deprecated_codes: vec!["zh_Hans", "zh", "cn"]
        },
        Language {
            name: "Chinese (Traditional)",
            code: "zh-Hant",
            native_name: "中文(繁體)",
            deprecated_codes: vec!["zh_Hant"]
        },
        Language {
            name: "isiZulu",
            code: "zu-ZA",
            native_name: "isiZulu",
            deprecated_codes: vec![]
        },
    ];
}

pub fn get_language(query: &str) -> Option<Language> {
    LANGUAGES
        .iter()
        .find(|lang| {
            lang.code == query || lang.deprecated_codes.contains(&query) || lang.name == query
        })
        .cloned()
}

pub(crate) fn serde_language<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let languages: Vec<String> = Vec::deserialize(d)?;
    Ok(languages
        .iter()
        .map(|lang| {
            match get_language(lang) {
                Some(lang) => lang.code,
                None => {
                    if lang.to_lowercase() == "neutral" {
                        "*"
                    } else {
                        lang
                    }
                }
            }
            .to_string()
        })
        .collect())
}
