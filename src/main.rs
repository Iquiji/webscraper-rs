use futures::{join, stream::iter, StreamExt, TryStreamExt};
use postgres::NoTls;
use select::document::Document;
use select::predicate::{Name, Predicate};
use std::sync::{atomic::AtomicU64, Arc};
use std::{collections::BTreeSet, error::Error};
use structopt::StructOpt;
use tokio::time::delay_for;
use url::Url;
use tokio_postgres::Statement;

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "webscraper-rs",
    version = "0.3.1",
    author = "Iquiji yt.failerbot.3000@gmail.com"
)]
struct Opt {
    /// Number of Threads
    #[structopt(short, long, default_value = "1")]
    n_workers: usize,
    /// How often to compute new Ranks/Weights in seconds
    #[structopt(short, long, default_value = "600")]
    rank_computing_delay: u64,
    /// Start Url
    #[structopt(short, long)]
    url: Option<String>,
    /// Only Compute Weights/Ranks
    #[structopt(short)]
    compute_only: bool,
    #[structopt(short, long)]
    verbose: bool,
    /// Duration between each print in ms
    #[structopt(short, long, default_value = "5000")]
    duration: u64,
    /// if images should be scraped
    #[structopt(short, long)]
    images: bool,
    /// if error's from scrape_url should be printed
    #[structopt(short, long)]
    print_main_errors: bool,
}

#[derive(Clone)]
struct PreparedStatements{
    get_websites : Statement,
    add_to_websites_v2 : Statement,
    into_crawl_queue : Statement,
    into_base_urls : Statement,
    update_crawl_queue_finished: Statement,
    error_in_scrape_url_handler: Statement,
    insert_into_images : Statement,
}
impl PreparedStatements{
    async fn new(db_client :&tokio_postgres::Client) -> Result<Self, Box<dyn Error>>{
        Ok(PreparedStatements {
            get_websites: db_client.prepare("UPDATE crawl_queue_v2 SET status = 'processing' WHERE url = ANY (SELECT url FROM crawl_queue_v2 WHERE status = 'processing' ORDER BY timestamp ASC NULLS FIRST,error_count ASC LIMIT 50) RETURNING * ;").await?,
            add_to_websites_v2: db_client.prepare("WITH before AS (SELECT * FROM websites_v2 WHERE url = $1 ), inserted AS (INSERT INTO websites_v2 (url,text,last_scraped,text_tsvector,hostname) VALUES ($1,$2,NOW(),to_tsvector('english',$2),$3) ON CONFLICT (url) DO UPDATE SET last_scraped = NOW(), text = $2 , text_tsvector = to_tsvector('english',$2) WHERE websites_v2.url = $1) SELECT last_scraped FROM before;").await?,
            into_crawl_queue: db_client.prepare("INSERT INTO crawl_queue_v2 (url,timestamp,status) VALUES ($1,NULL,$2) ON CONFLICT (url) DO NOTHING;").await?,
            into_base_urls : db_client.prepare("WITH inserted AS ( INSERT INTO base_url_links (base_url,target_url) VALUES ($1,$2) ON CONFLICT (base_url,target_url) DO NOTHING RETURNING target_url) UPDATE websites_v2 SET popularity = websites_v2.popularity + 1 FROM inserted WHERE hostname = inserted.target_url;").await?,
            update_crawl_queue_finished : db_client.prepare("UPDATE crawl_queue_v2 SET status = 'queued' , timestamp = current_timestamp WHERE url = $1;").await?,
            error_in_scrape_url_handler : db_client.prepare("UPDATE crawl_queue_v2 SET status = 'queued' , error_count = crawl_queue_v2.error_count + 1 WHERE url = $1").await?,
            insert_into_images : db_client.prepare("INSERT INTO images (url,text,text_tsvector) VALUES ($1,$2,to_tsvector('english',$2)) ON CONFLICT DO NOTHING;").await?,
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_args();
    //println!("{:?}", opt);
    let scraped_count: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let scraped_last_duration = Arc::new(AtomicU64::new(0));
    let mut scraped_avg: (u64, u64) = (0, 0);
    let error_count = Arc::new(AtomicU64::new(0));

    let (db_client, connection) =
        tokio_postgres::connect("host=db.failhack.com user=postgres port=5432 password=Rd7rko$g85GV^&%123", NoTls).await?;
    let db_client = Arc::new(db_client);

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    if opt.compute_only {
        println!("only computing Weights");
        compute_rank(true).await?;
        return Ok(());
    }

    let prepared_statements = PreparedStatements::new(&db_client).await?;

    let db_client_unfold = db_client.clone();
    let unfold_opts = opt.clone();
    let url_worker_fut = futures::stream::unfold( (),|()| async {
        let mut urls: Vec<url::Url> = vec![];
        let db_res = db_client_unfold.query(&prepared_statements.get_websites,&[]).await;
        match db_res {
            Ok(db_ok) => {
                // println!("{:?}",db_ok)
                for row in db_ok {
                    urls.push(Url::parse(row.get(0)).unwrap());
                }
            }
            Err(err) => {
                eprintln!("{}", err);
            }
        }
        if unfold_opts.verbose{
            println!("getting {} new urls",urls.len());
        }
        Some((iter(urls),()))
    }).flatten().map(|url| async {
        let url = url;
        match scrape_url(url.clone(),&db_client,unfold_opts.verbose,&prepared_statements,unfold_opts.images).await {
            Ok(_) => {
                scraped_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                scraped_last_duration.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            Err(err) => {
                if unfold_opts.verbose {
                    eprintln!("failed to scrape '{}' with error: {}",url.clone(),err);
                }
                // ignore db errors in error handling...
                let _ = db_client.execute(&prepared_statements.error_in_scrape_url_handler,&[&url.as_str()]).await;
                error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        };
    }).buffer_unordered(unfold_opts.n_workers).for_each(|_| async {});

    //weight updater task:
    //let db_weight_updater = db_client.clone();
    let db_weighter_opts = opt.clone();
    let weight_updater = tokio::spawn(async move {
        loop {
            delay_for(std::time::Duration::from_secs(
                db_weighter_opts.rank_computing_delay,
            ))
            .await;
            compute_rank(true).await.unwrap();
        }
    });

    // printing crawl speed and what was crawled :]
    let printer_scraped_last_duration = scraped_last_duration.clone();
    let printer_scraped_count = scraped_count.clone();
    let printer_error_count = error_count.clone();
    let printer_opts = opt.clone();
    let printer = tokio::spawn(async move {
        loop {
            delay_for(std::time::Duration::from_millis(printer_opts.duration)).await;
            let last_duration: u64 =
                printer_scraped_last_duration.swap(0, std::sync::atomic::Ordering::SeqCst);
            scraped_avg.0 += last_duration * (60000 / printer_opts.duration);
            scraped_avg.1 += 1;
            println!(
                "Scraped total: {}, Scraped per Minute: {:.1}, Avg SpM: {}, Scraped last duration: {}, Error count: {}",
                printer_scraped_count.load(std::sync::atomic::Ordering::SeqCst),
                last_duration * (60000 / printer_opts.duration),
                scraped_avg.0 / scraped_avg.1,
                last_duration,
                printer_error_count.load(std::sync::atomic::Ordering::Relaxed)
            );
        }
    });
    join!(url_worker_fut, weight_updater, printer);
    Ok(())
}

async fn scrape_url(
    url: url::Url,
    db_client: &tokio_postgres::Client,
    verbose: bool,
    prepared_statements: &PreparedStatements,
    images: bool,
) -> Result<(), Box<dyn Error>> {
    let host_str = url.host_str().ok_or(core::fmt::Error)?;
    let hostname = Url::parse(&(url.scheme().to_owned() + "://" + host_str + "/"))?;
    // let mut db_client = pool.get().unwrap(); // Why does this fail sometimes ?!
    if verbose{
        println!("Scraping: {}",&url);
    }

    // build Client:
    let web_client = reqwest::ClientBuilder::new().user_agent("Pwnsearch-Scraper/0.4").timeout(std::time::Duration::from_secs(60)).build()?;

    // let resp = reqwest::blocking::get(url.as_str())?;
    // let body = std::io::Read::take(resp,1024 * 1024 * 1024);
    let now = std::time::Instant::now();
    let resp = web_client.get(url.as_str()).send().await?;
    //let resp = reqwest::get(url.as_str()).await?;
    if verbose {
        //println!("content_length header: {:?}",&resp.content_length());
        println!("duration it took reqwest = {}ms",now.elapsed().as_millis());
    }
    let body: Vec<u8> = resp
        .bytes_stream()
        .try_fold(Vec::new(), |mut body, chunk| {
            if body.len() < 1024 * 1024 /* 1024 */{
                body.extend(&chunk);
            }
            async { Ok(body) }
        })
        .await?;

    let document = Document::from_read(&*body)?;

    let new_urls: BTreeSet<_> = document
        .find(Name("a"))
        .filter_map(|n| n.attr("href"))
        .filter_map(|x| {
            let res_url = if x.starts_with("http") {
                Url::parse(x)
            } else {
                url.join(x)
            };
            match res_url {
                Ok(_) => {}
                Err(_) => {
                    return None;
                }
            }
            let res_url = res_url.unwrap();
            let res_url_query = res_url.query();
            if res_url_query.is_some() {
                Some(
                    Url::parse(
                        &res_url
                            .as_str()
                            .to_owned()
                            .replace(res_url.query().unwrap(), ""),
                    )
                    .unwrap(),
                )
            } else {
                Some(res_url)
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
                .or(Name("h5"))
                .or(Name("title")),
        )
        .map(|f| {
            f.text()
                .replace("\n", "")
                .replace("\t", "")
                .replace("     ", "")
                .replace("<b>", "")
                .replace("</b>", "")
        })
        .collect();
    // see file:///Users/leon/.rustup/toolchains/stable-x86_64-apple-darwin/share/doc/rust/html/std/primitive.slice.html#method.join
    let mut text_string = "".to_string();
    for string in text.clone() {
        text_string.push_str(" ");
        text_string.push_str(&string);
    }
    if verbose{
        println!("text string for '{}' has len: {}, min with 10k: {}",&url,text_string.len(),text_string.chars().map(|_| 1).sum::<usize>().min(10000));
    }
    // url,alt-text
    if images{
        if verbose {
            println!("images!");
        }
        let all_images: Vec<(String,String)> = document.find(Name("img")).filter_map(|img| { 
            let img_url_res = img.attr("src");
            match img_url_res{
                Some(img_url) => {
                    let img_text = img.attr("alt");
                    let img_url_final = if img_url.starts_with("http") {
                        Url::parse(img_url)
                    } else {
                        url.join(img_url)
                    };

                    match img_url_final {
                        Ok(img_url) => {
                            //println!("image url: '{}' , text: '{}'",img_url.as_str().to_owned(),img_text.unwrap_or("").to_owned());
                            Some((img_url.as_str().to_owned(),img_text.unwrap_or("").to_owned()))
                        }
                        Err(_) => {
                            None
                        }
                    }
                }
                _ => {None}
            }
        }).collect();
        for image in all_images {
            if verbose {
                println!("image (link,text) pair{:?}",image);
            }
            db_client.execute(&prepared_statements.insert_into_images, &[&image.0,&image.1]).await?;
        }
    }
    // let re = regex::Regex::new(r"/\s\s+/g").unwrap();
    // text_string = (*re.replace_all(&text_string, " ")).to_owned();
    // MEM 'leak' after here:

    // ADD to websites_v2
    db_client.query(&prepared_statements.add_to_websites_v2,&[&url.as_str(),&(text_string.get(..(text_string.chars().map(|_| 1).sum::<usize>()).min(10000)).ok_or("")?.to_owned() + " " + url.as_str()),&hostname.as_str()]).await?;

    // make target base urls
    let target_base_urls: BTreeSet<Url> = new_urls
        .clone()
        .into_iter()
        .map(|target_url| {
            Url::parse(
                &(target_url.scheme().to_owned()
                    + "://"
                    + target_url.host_str().unwrap_or_else(|| "example.com")
                    + "/"),
            )
            .unwrap()
        })
        .collect();

    // insert into crawl_queue_v2 with timestamp NULL
    if verbose {
        println!("into crawl_queue_v2, num: {}",new_urls.len());   
    }
    for new_url in new_urls {
        db_client.execute(&prepared_statements.into_crawl_queue,&[&new_url.as_str(),&"queued"]).await?;
    }

    if verbose {
        println!("into base_url_links, num: {}",target_base_urls.len());
    }
    // insert into base_url_links
    for target_url in target_base_urls {
        if target_url.host_str().unwrap() == url.host_str().unwrap() {
            continue;
        } else if target_url.host_str().unwrap() == "example.com" {
            if verbose {
                eprintln!("Failed to parse host url because it contains 'example.com'");
            }
            continue;
        }
        if verbose {
            println!("{}",target_url);
        }
        let db_res = db_client.query(&prepared_statements.into_base_urls,
           &[&Url::parse(&(url.scheme().to_owned() + "://" + url.host_str().unwrap() + "/")).unwrap().as_str(),&target_url.as_str()]).await;
        match db_res {
            Ok(_) => {
                //println!("{:?}",db_ok);
            }
            Err(err) => {
                if verbose {
                    eprintln!("{}", err);
                }
                    continue;
            }
        }
    }

    // Update crawl_queue_v2 to say finishged crawling back in queue with timestamp now
    //db_client.execute(&prepared_statements.update_crawl_queue_finished,&[&url.as_str()]).await?;

    Ok(())
}

async fn compute_rank(on_db: bool) -> Result<(), Box<dyn Error>> {
    println!("Computing new Weights/Ranks...");
    if on_db {
        let (db_client, connection) =
        tokio_postgres::connect("host=free-db.coy5e9jykzwm.eu-central-1.rds.amazonaws.com user=postgres port=5432 password=Rd7rko$g85GV^&%123", NoTls).await?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
        db_client.execute(r#"WITH target_info AS(SELECT target_url,count(1)AS count,SUM(weight)AS total_weight FROM base_url_links GROUP BY target_url),updated_links AS(UPDATE base_url_links SET weight=(target_info.total_weight/count)*0.85 FROM target_info WHERE base_url=target_info.target_url)UPDATE websites_v2 SET"rank"=target_info.total_weight FROM target_info WHERE hostname = target_info.target_url;"#,&[]).await.unwrap();
        return Ok(());
    }
    Ok(())
    // let now = std::time::Instant::now();
    // let mut total_updated = 0;

    // let mut websites: BTreeSet<String> = BTreeSet::new();
    // match db_client.query(
    //     "SELECT * FROM websites_v2 ORDER BY last_scraped DESC LIMIT 1000",
    //     &[],
    // ) {
    //     Ok(val) => {
    //         for row in val {
    //             let url: String = row.get(0);
    //             websites.insert(url);
    //         }
    //     }
    //     Err(err) => {
    //         println!("{}", err);
    //         panic!();
    //     }
    // };
    // websites.into_iter().for_each(|url | {
    //     let mut all_weights : Vec<f64> = vec![];
    //     let mut all_weights_with_linkage : Vec<(String,String,f64)> = vec![];
    //     let url_as_url =Url::parse(&url).unwrap();
    //     let mut base_url : String = "".to_owned();

    //     // get all links where target_url == our website

    //     match db_client.query("SELECT weight,base_url,target_url FROM base_url_links WHERE target_url LIKE '%' || $1 ||'%';",&[&Url::parse(&(url_as_url.scheme().to_owned() + "://" + url_as_url.host_str().unwrap() + "/")).unwrap().as_str()]){
    //         Ok(val) => {
    //             for row in val {
    //                 let weight : f64 = row.get(0);
    //                 base_url = row.get(1);
    //                 let target_url : String = row.get(2);

    //                 all_weights.push(weight);

    //                 all_weights_with_linkage.push((base_url.clone(),target_url,weight));
    //             }
    //         }
    //         Err(err) => {
    //             panic!(err);
    //         }
    //     };

    //     // make weight of our website
    //     let total_weight = all_weights.into_iter().sum::<f64>() * 0.85;

    //     // get how many links to from website exist
    //     let how_many_links = db_client.query("SELECT COUNT(*) FROM base_url_links WHERE base_url = $1",&[&base_url]).unwrap();
    //     let mut linkage_count : i64 = 0;

    //     for row in how_many_links{
    //         linkage_count = row.get(0);
    //     }

    //     // get weight per link by total_weight/linkage count
    //     let weight_per_link : f64;
    //     if linkage_count != 0 {
    //         weight_per_link = total_weight / linkage_count as f64;
    //     }else{
    //         weight_per_link = 0.05
    //     }
    //     // set weight of all links where base url == our website to weight per link
    //     db_client.query("UPDATE base_url_links SET weight = $2 WHERE base_url = $1",&[&base_url,&weight_per_link]).unwrap();
    //     // set rank/weight of our website
    //     db_client.execute("UPDATE websites_v2 SET rank = $2 WHERE url = $1",&[&url,&total_weight]).unwrap();
    //     //println!("Setting url: {} to weight: {}, total of {} links get weight: {}",&url,total_weight,linkage_count,weight_per_link);
    //     total_updated += 1;
    // });
    // println!(
    //     "Updated {} Websites in {} seconds",
    //     total_updated,
    //     now.elapsed().as_secs()
    // );
}
