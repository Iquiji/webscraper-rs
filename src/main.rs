use crossbeam_channel::bounded;
use std::thread;
//use std::sync::{Arc,Mutex};
use postgres::{NoTls};
use select::document::Document;
use select::predicate::{Name};
use url::{Url};
use r2d2_postgres::PostgresConnectionManager;

fn main() {
    let (tx,rx) = bounded(500);

    let manager = PostgresConnectionManager::new(
        "host=free-db.coy5e9jykzwm.eu-central-1.rds.amazonaws.com user=postgres port=5432 password=Rd7rko$g85GV^&%123".parse().unwrap(),
        NoTls,
    );
    let pool = r2d2::Pool::new(manager).unwrap();

    // let response = db_client.query("Select url from crawl_queue order by priority desc limit 10",&[]).unwrap();
    // for row in response{
    //     let url : String = row.get(0);
    //     println!("{}",url);
    //     tx.send(url);
    // }
    tx.send(Url::parse("http://leonroth.de/").unwrap());

    let mut thread_handles: Vec<std::thread::JoinHandle<_>> = vec![];
    for _ in 0..5{
        let thread_tx = tx.clone();
        let thread_rx = rx.clone();
        let pool = pool.clone();

        let threadx = thread::spawn(move || {
            println!("Starting Up New Thread");
            loop{
                let mut db_client = pool.get().unwrap();
                let url : Url = thread_rx.recv().unwrap();
                println!("Scraping: {}",&url);
                let mut backfeed_num : usize = 0;
                let mut new_urls: Vec<Url> = vec![];
    
                // do something
                let body = reqwest::blocking::get(url.as_str()).unwrap();

                let document = Document::from_read(body).unwrap();
                document.find(Name("a"))
                .filter_map(|n| n.attr("href"))
                .for_each(|x|{
                    if x.starts_with("http"){
                        new_urls.push(Url::parse(x).unwrap())
                    }else{
                        new_urls.push(url.join(x).unwrap())
                    }
                });

                let text: Vec<String> = document.find(Name("p")).chain(document.find(Name("h1"))).chain(document.find(Name("h2"))).chain(document.find(Name("h3"))).chain(document.find(Name("h4"))).chain(document.find(Name("h5"))).map(|f| f.text().replace("\n", "").replace("\t","").replace("     ", "")).collect();
                let mut text_string = "".to_string();
                for string in text.clone(){
                    text_string.push_str(" ");
                    text_string.push_str(&string);
                }
                
                println!("Executing: {}",url);
                let db_res = db_client.execute("INSERT INTO websites_v2 (url,text,last_scraped) VALUES ($1,$2,NOW()) ON CONFLICT (url) DO UPDATE SET popularity = websites_v2.popularity + 1, last_scraped = NOW(), text = $2 WHERE websites_v2.url = $1;",&[&url.as_str(),&text_string.get(..100).to_owned()]);
                println!("{:?}",db_res);

                for new_url in new_urls{
                    if backfeed_num >= 10 || thread_tx.is_full(){
                        continue;
                    }else{
                        thread_tx.send(new_url);
                        backfeed_num += 1;
                    }
                }
            }

        });
        thread_handles.push(threadx);
    }
    for threadi in thread_handles {
        threadi.join();
    }
}