use crossbeam_channel::bounded;
use std::thread;
use std::sync::{Arc,atomic::AtomicU64};
use postgres::{NoTls};
use select::document::Document;
use select::predicate::{Name};
use url::{Url};
use r2d2_postgres::PostgresConnectionManager;
use chrono::NaiveDate;

static NTHREADS :usize = 16;

fn main() {
    let (tx,rx) = bounded(1000);
    let scraped_count: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let real_scraped_count = Arc::new(AtomicU64::new(0));
    let scraped_last_duration = Arc::new(AtomicU64::new(0));

    let manager = PostgresConnectionManager::new(
        "host=free-db.coy5e9jykzwm.eu-central-1.rds.amazonaws.com user=postgres port=5432 password=Rd7rko$g85GV^&%123".parse().unwrap(),
        NoTls,
    );
    let pool = r2d2::Pool::new(manager).unwrap();

    tx.send(Url::parse("http://leonroth.de/").unwrap()).unwrap();
    
    for _ in 0..NTHREADS{
        let thread_tx = tx.clone();
        let thread_rx = rx.clone();
        let pool = pool.clone();
        let scraped_count = scraped_count.clone();
        let scraped_last_duration = scraped_last_duration.clone();
        let real_scraped_count = real_scraped_count.clone();

        thread::spawn(move || {
            println!("Starting Up New Thread");
            loop{
                let mut db_client = pool.get().unwrap();
                let url : Url = thread_rx.recv().unwrap();
                //println!("Scraping: {}",&url);
                let mut backfeed_num : usize = 0;
                let mut new_urls: Vec<Url> = vec![];
                let mut scraped_today_flag = false;
    
                let body = reqwest::blocking::get(url.as_str()).unwrap();

                let document_res = Document::from_read(std::io::Read::take(body,1048576));
                let document: Document; 
                match document_res {
                    Ok(doc) =>{
                        document = doc;
                    }
                    Err(err) =>{
                        println!("{}",err);
                        continue;
                    }
                }
                
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
                
                let db_res= db_client.query("WITH before AS (SELECT * FROM websites_v2 WHERE url = $1 ), inserted AS (INSERT INTO websites_v2 (url,text,last_scraped) VALUES ($1,$2,NOW()) ON CONFLICT (url) DO UPDATE SET popularity = websites_v2.popularity + 1, last_scraped = NOW(), text = $2 WHERE websites_v2.url = $1) SELECT last_scraped FROM before;",&[&url.as_str(),&text_string.get(..100).to_owned()]);

                match &db_res {
                    Ok(ok_val) => {
                        for row in ok_val {
                            let date : NaiveDate  = row.get(0);
                            println!("Scraped_today: {}, Today {}, bool: {}",date, chrono::Utc::today().naive_utc(),date == chrono::Utc::today().naive_utc());
                            if date ==  chrono::Utc::today().naive_utc(){
                                scraped_today_flag = true;
                            }
                        }
                    },
                    Err(err) => {
                        println!("Error: {}",err)
                    }
                }

                for new_url in new_urls{
                    if backfeed_num >= 10 || thread_tx.is_full() || scraped_today_flag{
                        //println!("not sending in channel! backfeed: {}, tx.full: {}, scraped_today: {}",backfeed_num >= 5,thread_tx.is_full(),scraped_today_flag);
                        continue;
                    }else{
                        thread_tx.send(new_url).unwrap();
                        backfeed_num += 1;
                    }
                }
                scraped_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                scraped_last_duration.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if !scraped_today_flag{
                    real_scraped_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }

        });
    }
    loop{
        thread::sleep(std::time::Duration::from_millis(2000));
        let last_duration: u64 = scraped_last_duration.load(std::sync::atomic::Ordering::SeqCst);
        scraped_last_duration.store(0, std::sync::atomic::Ordering::SeqCst);

        println!("Scraped total: {}, Scraped Real: {}, Scraped per Minute: {}, Scraped last duration: {}",scraped_count.load(std::sync::atomic::Ordering::SeqCst),real_scraped_count.load(std::sync::atomic::Ordering::Relaxed),last_duration* 50,last_duration);
    }
}