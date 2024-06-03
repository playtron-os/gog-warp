use url::Url;

use crate::content_system::types::Endpoint;

/// Convert standard GOG url to partner link
pub fn gog_to_affiliate(gog_url: &str, channel_id: &str) -> Result<String, url::ParseError> {
    let mut url = Url::parse_with_params(gog_url, [("as", channel_id)])?;
    url.set_host(Some("af.gog.com"))?;
    Ok(url.as_str().to_string())
}

pub fn hash_to_galaxy_path(hash: &str) -> String {
    format!("{}/{}/{}", &hash[0..2], &hash[2..4], hash)
}

pub fn assemble_url(endpoint: &Endpoint, path: &str) -> String {
    let mut url = endpoint.url_format.clone();

    if endpoint.parameters().is_empty() {
        url.push('/');
        url.push_str(path);
        return url;
    }

    for (param, value) in endpoint.parameters.iter() {
        let mut url_param = String::from("{");
        url_param.push_str(param);
        url_param.push('}');

        let mut new_value = match value {
            serde_json::Value::String(v) => v.to_owned(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => "".to_string(),
        };

        if param == "path" {
            new_value.push('/');
            new_value.push_str(path);
        }

        url = url.replace(&url_param, &new_value);
    }

    url
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn gog_affiliate() {
        let url = "https://www.gog.com/game/alone_in_the_dark_the_trilogy_123?rel=idk";
        let affiliate_link =
            gog_to_affiliate(url, "123").expect("Failed to make the url affiliate");

        assert_eq!(
            affiliate_link,
            "https://af.gog.com/game/alone_in_the_dark_the_trilogy_123?rel=idk&as=123"
        )
    }

    #[test]
    fn hash() {
        assert_eq!(
            hash_to_galaxy_path("f1d41c76eb9639d2f8c1d3fd2057d7f1"),
            "f1/d4/f1d41c76eb9639d2f8c1d3fd2057d7f1"
        )
    }

    #[test]
    fn endpoint() {
        let sample = Endpoint {
            endpoint_name: "fastly".to_string(),
            url: "".to_string(),
            url_format: "{base_url}/token=nva={expires_at}~dirs={dirs}~token={token}{path}"
                .to_string(),
            parameters: HashMap::from_iter([
                (
                    "path".to_string(),
                    serde_json::Value::String("/content-system/v2/store/2034949552".to_string()),
                ),
                ("dirs".to_string(), serde_json::json!("4")),
                (
                    "token".to_string(),
                    serde_json::Value::String("0f76ef3e8f6b5d6baddc4".to_string()),
                ),
                (
                    "base_url".to_string(),
                    serde_json::Value::String("https://gog-cdn-fastly.gog.com".to_string()),
                ),
                ("expires_at".to_string(), serde_json::json!("1717011195")),
            ]),
            priority: 998,
            max_fails: 100,
            supports_generation: [1, 2].to_vec(),
            fallback_only: false,
        };

        let result = assemble_url(sample, "f1/d4/f1d41c76eb9639d2f8c1d3fd2057d7f1");
        assert_eq!(result, "https://gog-cdn-fastly.gog.com/token=nva=1717011195~dirs=4~token=0f76ef3e8f6b5d6baddc4/content-system/v2/store/2034949552/f1/d4/f1d41c76eb9639d2f8c1d3fd2057d7f1")
    }
}
