use chrono::NaiveDate;
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
    let (tx, rx) = bounded(1000);
    let scraped_count: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let real_scraped_count = Arc::new(AtomicU64::new(0));
    let scraped_last_duration = Arc::new(AtomicU64::new(0));

    let manager = PostgresConnectionManager::new(
        "host=free-db.coy5e9jykzwm.eu-central-1.rds.amazonaws.com user=postgres port=5432 password=Rd7rko$g85GV^&%123".parse().unwrap(),
        NoTls,
    );
    let pool = r2d2::Pool::new(manager).unwrap();

    tx.send(Url::parse("http://leonroth.de/").unwrap()).unwrap();

    for _ in 0..NTHREADS {
        let thread_tx = tx.clone();
        let thread_rx = rx.clone();
        let pool = pool.clone();
        let scraped_count = scraped_count.clone();
        let scraped_last_duration = scraped_last_duration.clone();
        let real_scraped_count = real_scraped_count.clone();

        thread::spawn(move || {
            println!("Starting Up New Thread");
            loop {
                let mut db_client = pool.get().unwrap();
                let url: Url = thread_rx.recv().unwrap();
                //println!("Scraping: {}",&url);
                let mut backfeed_num: usize = 0;
                let mut scraped_today_flag = false;

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
                    Ok(ok_val) => {
                        for row in ok_val {
                            let date: NaiveDate = row.get(0);
                            //println!("Scraped_today: {}, Today {}, bool: {}",date, chrono::Utc::today().naive_utc(),date == chrono::Utc::today().naive_utc());
                            if date == chrono::Utc::today().naive_utc() {
                                scraped_today_flag = true;
                            }
                        }
                    }
                    Err(err) => println!("Error: {}", err),
                }

                for new_url in new_urls {
                    //db_client.execute("INSERT INTO base_urls (url,link_urls) VALUES ($1,ARRAY[$2]) ON CONFLICT (url) DO UPDATE SET link_urls = array_append(base_urls.link_urls,$2) WHERE $2 <> ANY (base_urls.link_urls)",&[&url.host_str().unwrap(),&new_url.host_str().unwrap()]).unwrap();
                    if backfeed_num >= 10 || thread_tx.is_full() || scraped_today_flag {
                        //println!("not sending in channel! backfeed: {}, tx.full: {}, scraped_today: {}",backfeed_num >= 5,thread_tx.is_full(),scraped_today_flag);
                        continue;
                    } else {
                        thread_tx.send(new_url).unwrap();
                        backfeed_num += 1;
                    }
                }
                scraped_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                scraped_last_duration.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if !scraped_today_flag {
                    real_scraped_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        });
    }
    loop {
        thread::sleep(std::time::Duration::from_millis(2000));
        let last_duration: u64 = scraped_last_duration.swap(0, std::sync::atomic::Ordering::SeqCst);
        println!("Scraped total: {}, Scraped Real: {}, Scraped per Minute: {}, Scraped last duration: {}",scraped_count.load(std::sync::atomic::Ordering::SeqCst),real_scraped_count.load(std::sync::atomic::Ordering::Relaxed),last_duration* 30,last_duration);
    }
}
