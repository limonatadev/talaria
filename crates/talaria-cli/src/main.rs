use anyhow::{Result, anyhow};
use chrono::SecondsFormat;
use clap::{Parser, Subcommand, ValueEnum};
use prettytable::{Table, row};
use serde::Serialize;
use std::path::PathBuf;
use talaria_core::HermesClient;
use talaria_core::config::Config;
use talaria_core::images;
use talaria_core::models::*;
use talaria_core::supabase::SupabaseClient;

#[derive(Parser)]
#[command(name = "talaria", version)]
#[command(about = "CLI for the Hermes API (spec-driven)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Print current configuration (redacts API key).
    Config {
        #[command(subcommand)]
        cmd: ConfigCommands,
    },
    /// Health check
    Health {
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// HSUF enrich from images
    HsufEnrich(HsufArgs),
    /// Listings workflow
    Listings {
        #[command(subcommand)]
        cmd: ListingsCommands,
    },
    /// Jobs helpers
    Jobs {
        #[command(subcommand)]
        cmd: JobsCommands,
    },
    /// Pricing helpers
    Pricing {
        #[command(subcommand)]
        cmd: PricingCommands,
    },
    /// Usage reporting
    Usage {
        #[command(subcommand)]
        cmd: UsageCommands,
    },
    /// Image capture/upload helpers
    Images {
        #[command(subcommand)]
        cmd: ImagesCommands,
    },
}

#[derive(Parser)]
#[command(group(
    clap::ArgGroup::new("hsuf_images_source")
        .required(true)
        .args(&["images", "images_from_dir", "capture"])
))]
struct HsufArgs {
    #[arg(long, value_delimiter = ' ', conflicts_with_all = ["images_from_dir", "capture"])]
    images: Vec<String>,
    #[arg(long, conflicts_with_all = ["images", "capture"])]
    images_from_dir: Option<PathBuf>,
    #[arg(long, conflicts_with_all = ["images", "images_from_dir"])]
    capture: Option<usize>,
    #[arg(long, requires = "capture")]
    device: Option<u32>,
    #[arg(long)]
    sku: Option<String>,
    #[arg(long)]
    include_usage: bool,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show effective config
    Doctor,
}

#[derive(Subcommand)]
enum ListingsCommands {
    /// Create a listing
    Create(CreateListingArgs),
    /// Continue a listing with overrides
    Continue(ContinueListingArgs),
}

#[derive(Parser)]
#[command(group(
    clap::ArgGroup::new("listing_images_source")
        .required(true)
        .args(&["images", "images_from_dir", "capture"])
))]
struct CreateListingArgs {
    #[arg(long, value_delimiter = ' ', conflicts_with_all = ["images_from_dir", "capture"])]
    images: Vec<String>,
    #[arg(long, conflicts_with_all = ["images", "capture"])]
    images_from_dir: Option<PathBuf>,
    #[arg(long, conflicts_with_all = ["images", "images_from_dir"])]
    capture: Option<usize>,
    #[arg(long, requires = "capture")]
    device: Option<u32>,
    #[arg(long, required = true)]
    merchant_location_key: String,
    #[arg(long, required = true)]
    fulfillment_policy_id: String,
    #[arg(long, required = true)]
    payment_policy_id: String,
    #[arg(long, required = true)]
    return_policy_id: String,
    #[arg(long)]
    marketplace: Option<MarketplaceOpt>,
    #[arg(long)]
    publish: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    use_signed_urls: bool,
    #[arg(long)]
    sku: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
}

#[derive(Parser)]
struct ContinueListingArgs {
    #[arg(long, required = true)]
    sku: String,
    #[arg(long, required = true)]
    merchant_location_key: String,
    #[arg(long, required = true)]
    fulfillment_policy_id: String,
    #[arg(long, required = true)]
    payment_policy_id: String,
    #[arg(long, required = true)]
    return_policy_id: String,
    #[arg(long)]
    marketplace: Option<MarketplaceOpt>,
    #[arg(long, help = "JSON for CategorySelectionInput")]
    override_category: Option<String>,
    #[arg(long, num_args = 1.., value_delimiter = ' ')]
    override_resolved_images: Vec<String>,
    #[arg(long, num_args = 0..)]
    images: Vec<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum JobsCommands {
    /// Get job status
    Get {
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
enum PricingCommands {
    /// Price estimate for a listing request
    Quote(CreateListingArgs),
}

#[derive(Subcommand)]
enum UsageCommands {
    /// List usage
    List {
        #[arg(long)]
        org_id: Option<String>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
enum ImagesCommands {
    /// Capture images from a webcam
    Capture {
        #[arg(long, default_value_t = 1)]
        count: usize,
        #[arg(long)]
        device: Option<u32>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long)]
        upload: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Upload local image files to Supabase
    Upload {
        #[arg(long, num_args = 1.., value_delimiter = ' ', required = true)]
        paths: Vec<PathBuf>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Table,
}

#[derive(Clone, Copy, ValueEnum)]
enum MarketplaceOpt {
    #[value(name = "EBAY_US")]
    Us,
    #[value(name = "EBAY_UK")]
    Uk,
    #[value(name = "EBAY_DE")]
    De,
}

impl MarketplaceOpt {
    fn into_model(self) -> MarketplaceId {
        match self {
            MarketplaceOpt::Us => MarketplaceId::EbayUs,
            MarketplaceOpt::Uk => MarketplaceId::EbayUk,
            MarketplaceOpt::De => MarketplaceId::EbayDe,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;
    let client = HermesClient::new(config.clone())?;
    let supabase = images::supabase_from_config(&config)?;

    match cli.command {
        Commands::Config { cmd } => match cmd {
            ConfigCommands::Doctor => {
                let report = config.doctor();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("serializable doctor report")
                );
            }
        },
        Commands::Health { format } => {
            let resp = client.health().await?;
            emit_json_or_table(format, &resp, |_| {
                let mut table = Table::new();
                table.add_row(row!["service", resp.service]);
                table.add_row(row!["status", resp.status]);
                if let Some(version) = &resp.version {
                    table.add_row(row!["version", version]);
                }
                if let Some(git_sha) = &resp.git_sha {
                    table.add_row(row!["git_sha", git_sha]);
                }
                table
            });
        }
        Commands::HsufEnrich(args) => {
            let images = resolve_images_hsuf(&args, supabase.as_ref()).await?;
            let body = HsufEnrichRequest {
                images,
                sku: args.sku,
            };
            let resp = client.hsuf_enrich(&body, args.include_usage).await?;
            emit_json_or_table(args.format, &resp, |r| {
                let mut table = Table::new();
                table.add_row(row!["name", r.product.name]);
                if let Some(color) = &r.product.color {
                    table.add_row(row!["color", color]);
                }
                if let Some(sku) = &r.product.sku {
                    table.add_row(row!["sku", sku]);
                }
                if let Some(usage) = &r.usage {
                    table.add_row(row![
                        "usage",
                        format!(
                            "input_tokens={} output_tokens={}",
                            usage.input_tokens.unwrap_or_default(),
                            usage.output_tokens.unwrap_or_default()
                        )
                    ]);
                }
                table
            });
        }
        Commands::Listings { cmd } => match cmd {
            ListingsCommands::Create(args) => {
                let resolved_images = resolve_images_listing(&args, supabase.as_ref()).await?;
                let req = build_public_listing(&args, resolved_images)?;
                let resp = client.create_listing(&req).await?;
                emit_listing(args.format, &resp);
            }
            ListingsCommands::Continue(args) => {
                let req = build_continue_request(&args)?;
                let resp = client.continue_listing(&req).await?;
                emit_listing(args.format, &resp);
            }
        },
        Commands::Jobs { cmd } => match cmd {
            JobsCommands::Get { id, format } => {
                let resp = client.get_job_status(&id).await?;
                emit_json_or_table(format, &resp, job_table);
            }
        },
        Commands::Pricing { cmd } => match cmd {
            PricingCommands::Quote(args) => {
                let resolved_images = resolve_images_listing(&args, supabase.as_ref()).await?;
                let req = build_public_listing(&args, resolved_images)?;
                let resp = client.pricing_quote(&req).await?;
                emit_json_or_table(args.format, &resp, |quote| {
                    let mut table = Table::new();
                    table.add_row(row!["credits_estimated", quote.credits_estimated]);
                    if let Some(credits) = quote.credits_applied_cents {
                        table.add_row(row!["credits_applied_cents", credits]);
                    }
                    if let Some(net) = quote.net_due_cents {
                        table.add_row(row!["net_due_cents", net]);
                    }
                    table
                });
            }
        },
        Commands::Images { cmd } => match cmd {
            ImagesCommands::Capture {
                count,
                device,
                out_dir,
                upload,
                format,
            } => {
                let dir = out_dir.unwrap_or(std::env::temp_dir().join("talaria-captures"));
                let captured = if upload {
                    let supa = supabase
                        .as_ref()
                        .ok_or_else(|| anyhow!("Supabase config required for --upload"))?;
                    images::capture_and_upload(count, device, &dir, supa).await?
                } else {
                    talaria_core::camera::capture_many(count, device, &dir)?
                        .into_iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect()
                };
                emit_json_or_table(format, &captured, |items| {
                    let mut table = Table::new();
                    for item in items {
                        table.add_row(row!["image", item]);
                    }
                    table
                });
            }
            ImagesCommands::Upload { paths, format } => {
                let supa = supabase
                    .as_ref()
                    .ok_or_else(|| anyhow!("Supabase config required for uploads"))?;
                let urls = images::upload_paths(&paths, supa).await?;
                emit_json_or_table(format, &urls, |items| {
                    let mut table = Table::new();
                    for item in items {
                        table.add_row(row!["url", item]);
                    }
                    table
                });
            }
        },
        Commands::Usage { cmd } => match cmd {
            UsageCommands::List {
                org_id,
                from,
                to,
                format,
            } => {
                let resp = client.usage(org_id, from, to).await?;
                emit_json_or_table(format, &resp, |items| usage_table(items));
            }
        },
    }

    Ok(())
}

fn build_public_listing(
    args: &CreateListingArgs,
    images: Vec<String>,
) -> Result<PublicListingRequest> {
    let marketplace = args.marketplace.map(|m| m.into_model());
    let overrides = None;
    Ok(PublicListingRequest {
        dry_run: Some(args.dry_run),
        fulfillment_policy_id: args.fulfillment_policy_id.clone(),
        images_source: ImagesSource::Multiple(images),
        marketplace,
        merchant_location_key: args.merchant_location_key.clone(),
        overrides,
        payment_policy_id: args.payment_policy_id.clone(),
        publish: Some(args.publish),
        return_policy_id: args.return_policy_id.clone(),
        sku: args.sku.clone(),
        use_signed_urls: Some(args.use_signed_urls),
    })
}

fn build_continue_request(args: &ContinueListingArgs) -> Result<ContinueRequest> {
    let marketplace = args.marketplace.map(|m| m.into_model());
    let overrides = if args.override_category.is_some() || !args.override_resolved_images.is_empty()
    {
        let category = match &args.override_category {
            Some(raw) => Some(
                serde_json::from_str::<CategorySelectionInput>(raw)
                    .map_err(|err| anyhow!("override_category must be valid JSON: {err}"))?,
            ),
            None => None,
        };
        let resolved_images = if args.override_resolved_images.is_empty() {
            None
        } else {
            Some(args.override_resolved_images.clone())
        };
        Some(PublicPipelineOverrides {
            category,
            product: None,
            resolved_images,
        })
    } else {
        None
    };

    Ok(ContinueRequest {
        fulfillment_policy_id: args.fulfillment_policy_id.clone(),
        images_source: if args.images.is_empty() {
            None
        } else {
            Some(ImagesSource::Multiple(args.images.clone()))
        },
        marketplace,
        merchant_location_key: args.merchant_location_key.clone(),
        overrides,
        payment_policy_id: args.payment_policy_id.clone(),
        return_policy_id: args.return_policy_id.clone(),
        sku: args.sku.clone(),
    })
}

async fn resolve_images_hsuf(
    args: &HsufArgs,
    supabase: Option<&SupabaseClient>,
) -> Result<Vec<String>> {
    if !args.images.is_empty() {
        return Ok(args.images.clone());
    }
    if let Some(dir) = &args.images_from_dir {
        let supa = require_supabase(supabase)?;
        return images::upload_dir(dir, supa)
            .await
            .map_err(anyhow::Error::from);
    }
    if let Some(count) = args.capture {
        if count == 0 {
            return Err(anyhow!("capture count must be > 0"));
        }
        let supa = require_supabase(supabase)?;
        let dir = std::env::temp_dir().join("talaria-captures");
        return images::capture_and_upload(count, args.device, &dir, supa)
            .await
            .map_err(anyhow::Error::from);
    }
    Err(anyhow!("no images provided"))
}

async fn resolve_images_listing(
    args: &CreateListingArgs,
    supabase: Option<&SupabaseClient>,
) -> Result<Vec<String>> {
    if !args.images.is_empty() {
        return Ok(args.images.clone());
    }
    if let Some(dir) = &args.images_from_dir {
        let supa = require_supabase(supabase)?;
        return images::upload_dir(dir, supa)
            .await
            .map_err(anyhow::Error::from);
    }
    if let Some(count) = args.capture {
        if count == 0 {
            return Err(anyhow!("capture count must be > 0"));
        }
        let supa = require_supabase(supabase)?;
        let dir = std::env::temp_dir().join("talaria-captures");
        return images::capture_and_upload(count, args.device, &dir, supa)
            .await
            .map_err(anyhow::Error::from);
    }
    Err(anyhow!("no images provided"))
}

fn require_supabase<'a>(supa: Option<&'a SupabaseClient>) -> Result<&'a SupabaseClient> {
    supa.ok_or_else(|| anyhow!("Supabase config required for upload/capture workflows"))
}

fn emit_json_or_table<T: Serialize>(
    format: OutputFormat,
    value: &T,
    table_builder: impl FnOnce(&T) -> Table,
) {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(value).expect("serializable output");
            println!("{json}");
        }
        OutputFormat::Table => {
            let table = table_builder(value);
            table.printstd();
        }
    }
}

fn emit_listing(format: OutputFormat, resp: &ListingResponse) {
    emit_json_or_table(format, resp, |r| {
        let mut table = Table::new();
        table.add_row(row!["listing_id", r.listing_id]);
        table.add_row(row!["stages", ""]);
        for stage in &r.stages {
            table.add_row(row![
                format!("  {}", stage.name),
                format!(
                    "{} ms @ {}",
                    stage.elapsed_ms,
                    stage.timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
                )
            ]);
            if let Some(summary) = stage_output_summary(&stage.output) {
                table.add_row(row!["    summary", summary]);
            }
            if let Some(warnings) = stage_output_warnings(&stage.output)
                && !warnings.is_empty()
            {
                table.add_row(row!["    warnings", warnings.join("; ")]);
            }
        }
        table
    });
}

fn stage_output_summary(output: &serde_json::Value) -> Option<String> {
    output
        .get("summary")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn stage_output_warnings(output: &serde_json::Value) -> Option<Vec<String>> {
    let warnings = output.get("warnings")?;
    if let Some(values) = warnings.as_array() {
        let items = values
            .iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect::<Vec<_>>();
        return Some(items);
    }
    warnings.as_str().map(|value| vec![value.to_string()])
}

fn job_table(info: &JobInfo) -> Table {
    let mut table = Table::new();
    table.add_row(row!["id", info.id.clone()]);
    table.add_row(row![
        "created_at",
        info.created_at.to_rfc3339_opts(SecondsFormat::Secs, true)
    ]);
    table.add_row(row![
        "updated_at",
        info.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true)
    ]);
    match &info.state {
        JobState::Queued {} => {
            table.add_row(row!["state", "queued"]);
        }
        JobState::Running {} => {
            table.add_row(row!["state", "running"]);
        }
        JobState::Completed { result: _ } => {
            table.add_row(row!["state", "completed"]);
        }
        JobState::Failed { error, stage } => {
            table.add_row(row!["state", "failed"]);
            table.add_row(row!["error", error]);
            if let Some(stage) = stage {
                table.add_row(row!["stage", stage]);
            }
        }
    }
    table
}

fn usage_table(items: &[UsageSummary]) -> Table {
    let mut table = Table::new();
    table.add_row(row![
        "org_id",
        "credits",
        "listings_run",
        "jobs_enqueued",
        "window_from",
        "window_to"
    ]);
    for item in items {
        table.add_row(row![
            &item.org_id,
            item.counters.credits_consumed,
            item.counters.listings_run,
            item.counters.jobs_enqueued,
            item.window_from
                .map(|d| d.to_rfc3339_opts(SecondsFormat::Secs, true))
                .unwrap_or_else(|| "-".into()),
            item.window_to
                .map(|d| d.to_rfc3339_opts(SecondsFormat::Secs, true))
                .unwrap_or_else(|| "-".into())
        ]);
    }
    table
}
