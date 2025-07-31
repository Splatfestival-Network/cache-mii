use dotenvy::dotenv;
use rocket::http::Status;
use rocket::fs::NamedFile;
use rocket::State;
use reqwest::Client;
use bytes::Bytes;

static MII_RENDERER_URL: &str = "https://mii-render.spfn.net";

static ALLOWED_FORMATS: [&str; 2] = [
    "png",
    "tga",
];

async fn request_mii_render(client: &State<Client>, mii_data: &str, ext: &str) -> Option<Bytes> {
    let req_str = format!("{}/miis/image.{}?data={}", MII_RENDERER_URL, ext, mii_data);

    let response = client.get(req_str)
        .send()
        .await
        .ok()?;

    Some(response.bytes().await.expect("Failed to parse Mii Render to Bytes"))
}

#[rocket::get("/<pid_str>/<path>")]
pub async fn mii_render(pid_str: &str, path: &str, client: &State<Client>) -> Option<NamedFile> {
    let ext = path.splitn(2, ".").nth(1)?;
    let pid: i32 = pid_str.parse().expect("PID is not an i32");

    if !ALLOWED_FORMATS.contains(&ext) {return None}

    let mii_data = {
        let route = format!("https://account.spfn.net/api/v2/users/{}/mii", pid);
        let response = client.get(route)
            .send()
            .await
            .ok()
            .expect("Error Retrieving Mii Data");

        let data: String = response.json().await.expect("Failed to parse Mii Data Response");

        data
    };

    let cache_path = std::path::Path::new("miis/").join(pid.to_string());
    let img_path = cache_path.clone().join(path);
    let data_path = cache_path.clone().join("data");

    // Update Cache
    if cache_path.exists() && cache_path.is_dir() { // Mii data was previously cached - Verify it is correct
        if data_path.exists() && data_path.is_file() { // Data exists - Check if it's the same
            let cache_data = std::fs::read_to_string(&data_path);

            match cache_data {
                Ok(hash) => {
                    if hash == mii_data { // Cached images are correct
                        if !img_path.exists() || !img_path.is_file() { // File type was not found - Request data
                            let img_data = request_mii_render(client, &mii_data, ext).await?;
                            let _ = std::fs::write(&img_path, &img_data);
                        }
                    } else { // Cached images are outdated - Update data
                        let img_data = request_mii_render(client, &mii_data, ext).await?;
                        let _ = std::fs::write(&img_path, &img_data);
                    };
                }

                Err(_) => { // Data cannot be read - Request data
                    println!("Data is not valid");
                    let _ = std::fs::write(data_path, &mii_data);

                    let img_data = request_mii_render(client, &mii_data, ext).await?;
                    let _ = std::fs::write(&img_path, &img_data);
                }
            }
        } else { // Data isn't saved - Request data
            println!("Data is not recorded");
            let _ = std::fs::write(data_path, &mii_data);

            let img_file = request_mii_render(client, &mii_data, ext).await?;
            let _ = std::fs::write(&img_path, &img_file);
        }
    } else { // Mii data was never cached - Request data
        println!("Mii Data was Never Cached");
        let _ = std::fs::create_dir(format!("miis/{}", pid.to_string()));

        let _ = std::fs::write(data_path, &mii_data);

        let img_data = request_mii_render(client, &mii_data, ext).await?;
        let _ = std::fs::write(&img_path, &img_data);
    };

    let img = NamedFile::open(img_path).await.map_err(|_| Status::InternalServerError).expect("Error opening Image");

    Some(img)
}

#[rocket::launch]
async fn launch() -> _ {
    dotenv().ok();

    let client = Client::new();

    let mii_path = std::path::Path::new("miis");
    if !mii_path.exists() || !mii_path.is_dir() {
        let _ = std::fs::create_dir("miis");
    }

    rocket::build()
        .manage(client)
        .mount("/", rocket::routes![mii_render])
}