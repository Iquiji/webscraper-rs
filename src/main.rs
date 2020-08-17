use crossbeam_channel::bounded;
use postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use select::document::Document;
use select::predicate::{Name, Predicate};
use std::sync::{atomic::AtomicU64, Arc};
use std::{collections::BTreeSet, thread};
use url::Url;

static NTHREADS: usize = 16;

// TODO: make table for scraped sites with timestamp || make table for base url relations and compute 'PageRank' :]
fn main() {
    let (tx, rx) = bounded(100);
    let scraped_count: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let scraped_last_duration = Arc::new(AtomicU64::new(0));

    let manager = PostgresConnectionManager::new(
        "host=free-db.coy5e9jykzwm.eu-central-1.rds.amazonaws.com user=postgres port=5432 password=Rd7rko$g85GV^&%123".parse().unwrap(),
        NoTls,
    );
    let pool = r2d2::Pool::new(manager).unwrap();

    tx.send(Url::parse("http://leonroth.de/").unwrap()).unwrap();

    for _ in 0..NTHREADS {
        let thread_rx = rx.clone();
        let pool = pool.clone();
        let scraped_count = scraped_count.clone();
        let scraped_last_duration = scraped_last_duration.clone();

        thread::spawn(move || {
            println!("Starting Up New Thread");
            for url in thread_rx {
                let mut db_client = pool.get().unwrap();
                //println!("Scraping: {}",&url);

                let body = reqwest::blocking::get(url.as_str()).unwrap();

                let document_res = Document::from_read(std::io::Read::take(body, 1048576));
                let document: Document;
                match document_res {
                    Ok(doc) => {
                        document = doc;
                    }
                    Err(err) => {
                        println!("{}", err);
                        continue;
                    }
                }

                let new_urls: BTreeSet<_> = document
                    .find(Name("a"))
                    .filter_map(|n| n.attr("href"))
                    .map(|x| {
                        if x.starts_with("http") {
                            Url::parse(x).unwrap()
                        } else {
                            url.join(x).unwrap()
                        }
                    })
                    .collect();

                let text: Vec<String> = document
                    .find(
                        Name("p")
                            .or(Name("h1"))
                            .or(Name("h2"))
                            .or(Name("h3"))
                            .or(Name("h4"))
                            .or(Name("h5")),
                    )
                    .map(|f| {
                        f.text()
                            .replace("\n", "")
                            .replace("\t", "")
                            .replace("     ", "")
                    })
                    .collect();
                // see file:///Users/leon/.rustup/toolchains/stable-x86_64-apple-darwin/share/doc/rust/html/std/primitive.slice.html#method.join
                let mut text_string = "".to_string();
                for string in text.clone() {
                    text_string.push_str(" ");
                    text_string.push_str(&string);
                }

                let db_res= db_client.query("WITH before AS (SELECT * FROM websites_v2 WHERE url = $1 ), inserted AS (INSERT INTO websites_v2 (url,text,last_scraped) VALUES ($1,$2,NOW()) ON CONFLICT (url) DO UPDATE SET popularity = websites_v2.popularity + 1, last_scraped = NOW(), text = $2 WHERE websites_v2.url = $1) SELECT last_scraped FROM before;",&[&url.as_str(),&text_string.get(..100).to_owned()]);
                match &db_res {
                    Ok(_) => {}
                    Err(err) => println!("Error: {}", err),
                }

                for new_url in new_urls {
                    db_client.query("INSERT INTO crawl_queue_v2 (url,timestamp,status) VALUES ($1,current_timestamp - interval '1 day' ,$2) ON CONFLICT (url) DO NOTHING",&[&new_url.as_str(),&"queued"]).unwrap();
                }

                let db_res = db_client.execute("UPDATE crawl_queue_v2 SET status = 'queued' , timestamp = current_timestamp WHERE url = $1",&[&url.as_str()]);
                match &db_res {
                    Ok(_) => {}
                    Err(err) => println!("Error: {}", err),
                }

                scraped_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                scraped_last_duration.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });
    }
    // getting new Urls from Table 'crawl_queue_v2' :]
    thread::spawn(move || {
        let mut db_client = pool.clone().get().unwrap();
        let thread_tx = tx.clone();
        loop {
            let db_res = db_client.query("UPDATE crawl_queue_v2 SET status = 'processing' WHERE url IN (SELECT url FROM crawl_queue_v2 WHERE status = 'queued' ORDER BY timestamp ASC LIMIT 50) RETURNING * ;",&[]);
            match db_res {
                Ok(db_ok) => {
                    // println!("{:?}",db_ok)
                    for row in db_ok {
                        thread_tx.send(Url::parse(row.get(0)).unwrap()).unwrap();
                    }
                }
                Err(err) => {
                    println!("{}", err);
                    continue;
                }
            }
            println!("getting new for sending!");
        }
    });
    // printing crawl speed and what was crawled :]
    loop {
        thread::sleep(std::time::Duration::from_millis(2000));
        let last_duration: u64 = scraped_last_duration.swap(0, std::sync::atomic::Ordering::SeqCst);
        println!(
            "Scraped total: {}, Scraped per Minute: {}, Scraped last duration: {}",
            scraped_count.load(std::sync::atomic::Ordering::SeqCst),
            last_duration * 30,
            last_duration
        );
    }
}
