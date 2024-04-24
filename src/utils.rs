use url::Url;

/// Convert standard GOG url to partner link
pub fn gog_to_affiliate(gog_url: &str, channel_id: &str) -> Result<String, url::ParseError> {
    let mut url = Url::parse_with_params(gog_url, [("as", channel_id)])?;
    url.set_host(Some("af.gog.com"))?;
    Ok(url.as_str().to_string())
}

pub fn hash_to_galaxy_path(hash: &str) -> String {
    format!("{}/{}/{}", &hash[0..2], &hash[2..4], hash)
}

#[cfg(test)]
mod tests {
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
}
