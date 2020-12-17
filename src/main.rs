#![allow(dead_code)]
#![deny(warnings)]
#![deny(unsafe_code)]

extern crate reqwest;
extern crate scraper;
extern crate select;

use std::{cell::RefCell, fs::File, io::BufWriter, path::PathBuf, rc::Rc};

use futures::{stream, StreamExt};
use select::document::Document;
use select::predicate::Class;

// Name your user agent after your app?
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[tokio::main]
async fn main() {
		let client = reqwest::Client::builder()
				.user_agent(APP_USER_AGENT)
				.build()
				.unwrap();
		get_4chan_wg(&client).await.unwrap();
}

async fn get_4chan_wg(client: &reqwest::Client) -> Result<(), Box<dyn std::error::Error>> {
		let response = client.get("https://boards.4chan.org/wg/").send().await?;
		assert!(response.status().is_success());

		let body = response.text().await?;
		let threads: Vec<String> = Document::from_read(body.as_bytes())
				.unwrap()
				.find(Class("thread"))
				.filter_map(|thread| {
						if thread.attr("id").unwrap() != "t6872254" {
								thread.attr("id")
						} else {
								None
						}
				})
				.map(|thread| {
						format!(
								"https://boards.4chan.org/wg/thread/{}",
								thread.chars().skip(1).collect::<String>()
						)
				})
				.collect();

		let bodies = stream::iter(threads)
				.map(|thread_url| {
						let client = client.clone();
						tokio::spawn(async move {
								let resp = client.get(&thread_url).send().await?;
								resp.text().await
						})
				})
				.buffer_unordered(8);

		let images: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

		bodies
				.for_each(|body| async {
						match body {
								Ok(Ok(b)) => {
										for i in extract_images_4chan(&b) {
												// Add image link to images vector
												images.borrow_mut().push(i);
										}
								}
								Ok(Err(e)) => eprintln!("Got an error: {}", e),
								Err(e) => eprintln!("Got an error: {}", e),
						}
				})
				.await;

		match download_from_urls(images.borrow().clone(), PathBuf::from("downloads")).await {
			Ok(_) => (),
			Err(e) => eprintln!("{:?}", e),
		};

		Ok(())
}

fn extract_images_4chan<'a>(body: &'a str) -> Vec<String> {
		Document::from_read(body.as_bytes())
				.unwrap()
				.find(Class("fileThumb"))
				.filter_map(|image| {
					image.attr("href")
				})
				.map(|image_url| format!("https:{}", image_url))
				.collect()
}

use std::io::prelude::*;
async fn download_from_urls<'a>(urls: Vec<String>, path: PathBuf) -> std::io::Result<()> {
		for url in urls {
			let mut writer = BufWriter::new(
				File::create(&path.join(url.split('/').last().unwrap()))?
			);
			writer.write(&reqwest::get(&url).await.unwrap().bytes().await.unwrap())?;	
		}
		Ok(())
}
