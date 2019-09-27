use std::io;
use std::path::Path;
use std::time;

use clap::Arg;
use failure::bail;
use failure::err_msg;
use failure::format_err;
use failure::ResultExt;
use std::io::Write;

mod period;

fn main() -> Result<(), failure::Error> {
    let matches = clap::App::new(clap::crate_name!())
        .arg(
            Arg::with_name("headers")
                .short("H")
                .long("header")
                .takes_value(true)
                .number_of_values(1)
                .multiple(true),
        )
        .arg(
            Arg::with_name("min-age")
                .long("min-age")
                .takes_value(true)
                .number_of_values(1),
        )
        .arg(Arg::with_name("url").index(1).required(true))
        .arg(Arg::with_name("output").index(2).required(true))
        .version(clap::crate_version!())
        .get_matches();

    let url = matches.value_of("url").expect("required");
    let output = Path::new(matches.value_of_os("output").expect("required"));

    let min_age = match matches.value_of("min-age") {
        Some(v) => Some(
            period::parse_duration(v).with_context(|_| format_err!("parsing min-age: {:?}", v))?,
        ),
        None => None,
    };

    let metadata_before = match output.metadata() {
        Ok(metadata) => Some(metadata),
        Err(ref e) if io::ErrorKind::NotFound == e.kind() => None,
        Err(e) => Err(e).with_context(|_| format_err!("reading output's info: {:?}", output))?,
    };

    let now = chrono::Utc::now();

    let mtime_before = metadata_before
        .as_ref()
        // errors only for unsupported platforms, apparently
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t))
        // discard mtimes in the future, which are generally unexpected
        .filter(|t| t < &now);

    if let Some(mtime) = mtime_before {
        if let Some(min_age) = min_age {
            if mtime > now - min_age {
                return Ok(());
            }
        }
    }

    // no point doing any networking if we aren't going to be able to store the result
    let temp = {
        let output_location = output.parent().unwrap_or_else(|| Path::new("/"));
        tempfile_fast::PersistableTempFile::new_in(output_location)
            .with_context(|_| format_err!("creating temporary file in {:?}", output_location))?
    };

    let mut temp = io::BufWriter::new(temp);

    let mut req = ureq::get(url);
    req.redirects(10);

    if let Some(mtime) = mtime_before {
        req.set("If-Modified-Since", &mtime.to_rfc2822());
    }

    if let Some(headers) = matches.values_of("headers") {
        for header in headers {
            let colon = header.find(':').ok_or_else(|| {
                format_err!(
                    "header missing a colon, expected format 'Foo: bar', got {:?}",
                    header
                )
            })?;

            let (key, mut value) = header.split_at(colon);

            // colon
            value = &value[1..];

            if value.starts_with(' ') {
                value = &value[1..];
            }

            req.set(key, value);
        }
    }

    let response = req.call();

    if let Some(err) = response.synthetic_error() {
        Err(ureq_error(err)).with_context(|_| err_msg("requesting"))?;
    }

    match response.status() {
        200..=299 => (),
        304 /* not modified */ => return Ok(()),
        300..=399 => bail!("confused by redirection: {:?}", response.status_line()),
        400..=599 => bail!("unhappy response: {:?}", response.status_line()),
        _ => bail!("unexpected response: {:?}", response.status_line()),
    }

    let server_date = if let Some(server_modified) = response.header("Last-Modified") {
        chrono::DateTime::parse_from_rfc2822(server_modified)
            .ok()
            .map(time::SystemTime::from)
    } else {
        None
    };

    io::copy(&mut response.into_reader(), &mut temp).with_context(|_| err_msg("downloading"))?;

    temp.flush()
        .with_context(|_| err_msg("completing download"))?;

    let temp = temp.into_inner().expect("just flushed");

    if let Some(server_date) = server_date {
        let _ = filetime::set_file_handle_times(
            temp.as_ref(),
            None,
            Some(filetime::FileTime::from(server_date)),
        );
    }

    match temp.persist_by_rename(output) {
        Ok(()) => (),
        Err(e) => {
            Err(e.error).with_context(|_| format_err!("replacing {:?} with download", output))?
        }
    };

    Ok(())
}

fn ureq_error(err: &ureq::Error) -> failure::Error {
    format_err!("request failed: {:?}", err)
}
