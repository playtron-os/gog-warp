# gog-warp

A Rust library for interfacing with GOG API.

## Current features

Currently library is focused on supporting Windows builds available through GOG Galaxy API.
Proper support for MacOS games can be easily added in the future.

The library currently handles:

- Authorization
- Getting user details
- Getting list of owned games 
- Downloading and Updating games and DLC (requires downloader feature to be enabled)
- Downloading game dependencies

## Quick Start

Take a look at [examples](examples/get_library.rs)

Below is a example of initial authorization flow 

```rust
use std::fs::File;
use std::io::{stdin, stdout, BufRead, Write};

#[tokio::main]
async fn main() {
    println!("Open the link in your browser");
    println!("https://auth.gog.com/auth?client_id=46899977096215655&redirect_uri=https://embed.gog.com/on_login_success?origin=client&response_type=code&layout=client2");
    print!("Paste the authorization code from the resulting url: ");
    stdout().flush().unwrap();
    let mut stdin = stdin().lock();
    let mut code: String = String::new();
    stdin.read_line(&mut code).expect("Failed to read stdin");

    // Initialize the warp core
    // This is the top level manager of auth and entry point for communication with GOG
    let core = gog_warp::Core::new();
    // We can clone the core, they will share the token state
    let core2 = core.clone();

    // Finish the auth flow and get the token
    core.get_token_with_code(code)
        .await
        .expect("Failed to get auth code");

    // A small utility to check if appropriate token is in place
    assert!(core.ensure_auth().is_ok(), "Login wasn't successful");
    // Cloned core shares the same state 
    assert!(core2.ensure_auth().is_ok(), "Login wasn't successful");

    println!("Login success");
    // Deserialize internal tokens HashMap for storage
    // For example purposes, let's save the token to cwd
    let data = core.serialize_tokens().unwrap();
    let mut file = File::create(".gog.token").expect("Failed to create file");
    file.write_all(data.as_bytes())
        .expect("Failed to write data");
    // Use core.deserialize_tokens() to load them in
}

```

## License
[Apache-2.0 license](LICENSE)
