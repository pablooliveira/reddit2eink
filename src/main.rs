use clap::Clap;
use roux::subreddit::responses::comments::SubredditCommentsData;
use roux::subreddit::responses::comments::SubredditReplies;
use roux::subreddit::responses::submissions::SubmissionsData;
use roux::util::error::RouxError;
use roux::Subreddit;
use shell_words;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tokio;

#[derive(Clap)]
#[clap(version = "0.0.1", author = "Pablo Oliveira <pablo@sifflez.org>")]
/// Read latest posts and comments on your favorite subreddit on an eink device.
///
/// Download latests posts from a subreddit with the full comment tree
/// to a markdown document.
/// If ebook-convert is available, an ebook can be optionally produced.
struct Opts {
    /// Subreddit to retrieve posts from (without /r/)
    subreddit: String,
    /// Output file
    output: String,
    /// Number of posts to retrieve
    #[clap(short, long, default_value = "10")]
    posts: u32,
    /// ebook-convert path
    #[clap(short, long, default_value = "/usr/bin/ebook-convert")]
    ebook_convert: String,
    /// extra arguments for ebook-convert
    #[clap(
        short,
        long,
        default_value = "--chapter \"//h:h1\" --smarten-punctuation --markdown-extensions meta"
    )]
    converter_args: String,
    /// verbose output
    #[clap(short, long)]
    verbose: bool,
}

fn quote(str: &str) -> String {
    let mut s = String::from(">");
    s.push_str(str);
    return s.replace("\n", "\n>");
}

fn parse_comment(comment: &SubredditCommentsData, depth: u32) -> String {
    let mut output = String::from("\n\n");
    if let Some(author) = comment.author.as_ref() {
        output.push_str("** ");
        output.push_str(author);
        output.push_str(" -- **\n");
        output.push_str(comment.body.as_ref().unwrap());
        output.push_str("\n");
        match &comment.replies {
            Some(SubredditReplies::Reply(replies)) => {
                for reply in &replies.data.children {
                    let rep_output = parse_comment(&reply.data, depth + 1);
                    output.push_str(&rep_output);
                }
            }
            Some(SubredditReplies::Str(_)) | None => (),
        };
    }
    return quote(&output);
}

async fn parse_post<'a>(
    subreddit: &Subreddit,
    post: &'a SubmissionsData,
) -> Result<String, RouxError> {
    let mut output = String::from("#");
    output.push_str(&post.title);
    output.push_str("\n");
    output.push_str(&post.selftext);

    let comments = subreddit.article_comments(&post.id, None, None).await?;
    for comment in &comments.data.children {
        output.push_str(&parse_comment(&comment.data, 0));
    }
    return Ok(output);
}

fn clean_markdown(str: &str) -> String {
    return str.replace("&amp;#x200B;", "\n");
}

fn write_markdown_file(path: &Path, output: &str) -> io::Result<()> {
    let clean = clean_markdown(output);
    let mut out_stream = File::create(path)?;
    out_stream.write_all(clean.as_bytes())?;
    Ok(())
}

fn run_ebook_converter(md_path: &Path, opts: &Opts) -> io::Result<()> {
    let extra_args =
        shell_words::split(&opts.converter_args).expect("cannot parse convert arguments");
    let output = Command::new(&opts.ebook_convert)
        .arg(&md_path.to_str().unwrap())
        .arg(&opts.output)
        .args(extra_args)
        .output()?;

    if opts.verbose {
        println!("ebook-convert status: {}", output.status);
        println!("{}", String::from_utf8_lossy(&output.stdout));
        println!("{}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), RouxError> {
    let opts: Opts = Opts::parse();
    let mut output = String::from(format!(
        "---
title: /r/{}
---

",
        opts.subreddit
    ));

    let subreddit = Subreddit::new(&opts.subreddit);
    let latest = subreddit.latest(opts.posts, None).await?;
    for post in latest.data.children {
        output.push_str(&parse_post(&subreddit, &post.data).await?);
    }

    let path = Path::new(&opts.output);
    let md_path = path.with_extension("md");

    write_markdown_file(&md_path, &output).expect("cannot write markdown to output file");

    if path.extension() != Some(OsStr::new("md")) {
        run_ebook_converter(&md_path, &opts).expect("Cannot run ebook-convert command");
    }
    Ok(())
}
