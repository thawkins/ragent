with open('src/main.rs', 'r') as f:
    lines = f.readlines()

# Insert after line 933 (0-indexed: 932) which is '          }' closing Some(Commands::Memory)
# Line 933 is the '          }' at the end of Some(Commands::Memory { command }) => { ... }
insert_idx = 933  # After the closing brace of Memory block

spec_handler = '''          Some(Commands::Spec { command }) => {
              let working_dir = std::env::current_dir().unwrap_or_default();
              let specs_root = working_dir.join("specs");
              let mgr = ragent_specs::SpecManager::new(&specs_root);
              match command {
                  SpecCommands::Create { specname, feature } => {
                      let id = match ragent_specs::spec::SpecId::new(&specname) {
                          Some(id) => id,
                          None => {
                              eprintln!("Invalid spec ID: {}", specname);
                              return Ok(());
                          }
                      };
                      let spec_md = ragent_specs::SpecTemplate::generate(&id, &feature);
                      let plan_md = ragent_specs::PlanTemplate::generate(&id, &feature);
                      match ragent_specs::SpecIo::create_spec_dir(&specs_root, &id, &spec_md, &plan_md).await {
                          Ok(dir) => println!("Created spec at {}", dir.display()),
                          Err(e) => eprintln!("Failed to create spec: {}", e),
                      }
                  }
                  SpecCommands::List { status, prefix, archived } => {
                      let mut filter = ragent_specs::SpecFilter::new();
                      if let Some(s) = status {
                          if let Some(st) = ragent_specs::spec::SpecStatus::parse(&s) {
                              filter = filter.with_status(st);
                          }
                      }
                      if let Some(p) = prefix {
                          filter = filter.with_id_prefix(p);
                      }
                      if archived {
                          filter = filter.with_archived();
                      }
                      match mgr.list_specs(&filter).await {
                          Ok(specs) => {
                              if specs.is_empty() {
                                  println!("No specs found.");
                              } else {
                                  println!("{:<20} {:12} {}", "ID", "Status", "Title");
                                  for spec in specs {
                                      println!("{:<20} {:12} {}", spec.id.as_str(), spec.status.as_str(), spec.title);
                                  }
                              }
                          }
                          Err(e) => eprintln!("Failed to list specs: {}", e),
                      }
                  }
                  SpecCommands::Search { query } => {
                      match mgr.search_specs(&query).await {
                          Ok(results) => {
                              if results.is_empty() {
                                  println!("No specs found matching '{}.'", query);
                              } else {
                                  for r in results {
                                      println!("{} (score: {}, status: {}) — {}", r.spec.id, r.score, r.spec.status, r.spec.title);
                                  }
                              }
                          }
                          Err(e) => eprintln!("Search failed: {}", e),
                      }
                  }
                  SpecCommands::Validate { spec_id } => {
                      let specs = if let Some(id_str) = spec_id {
                          match ragent_specs::spec::SpecId::new(&id_str) {
                              Some(id) => {
                                  match mgr.read_spec(&id).await {
                                      Ok(spec) => vec![spec],
                                      Err(e) => {
                                          eprintln!("Failed to read spec: {}", e);
                                          return Ok(());
                                      }
                                  }
                              }
                              None => {
                                  eprintln!("Invalid spec ID: {}", id_str);
                                  return Ok(());
                              }
                          }
                      } else {
                          match mgr.discover_specs().await {
                              Ok(specs) => specs,
                              Err(e) => {
                                  eprintln!("Discovery failed: {}", e);
                                  return Ok(());
                              }
                          }
                      };
                      let mut has_errors = false;
                      for spec in specs {
                          let report = ragent_specs::validate(&spec);
                          println!("{}", report.format(spec.id.as_str()));
                          if report.has_errors() {
                              has_errors = true;
                          }
                      }
                      if has_errors {
                          std::process::exit(1);
                      }
                  }
                  SpecCommands::Status { spec_id, new_status } => {
                      let id = match ragent_specs::spec::SpecId::new(&spec_id) {
                          Some(id) => id,
                          None => {
                              eprintln!("Invalid spec ID: {}", spec_id);
                              return Ok(());
                          }
                      };
                      let mut spec = match mgr.read_spec(&id).await {
                          Ok(s) => s,
                          Err(e) => {
                              eprintln!("Failed to read spec: {}", e);
                              return Ok(());
                          }
                      };
                      if let Some(new_status_str) = new_status {
                          let new_status = match ragent_specs::spec::SpecStatus::parse(&new_status_str) {
                              Some(s) => s,
                              None => {
                                  eprintln!("Unknown status: {}", new_status_str);
                                  return Ok(());
                              }
                          };
                          match mgr.transition(&mut spec, new_status, "cli").await {
                              Ok(()) => println!("Transitioned {} to {}", spec_id, new_status.as_str()),
                              Err(e) => eprintln!("Transition failed: {}", e),
                          }
                      } else {
                          let next = ragent_specs::manager::next_statuses(spec.status);
                          let next_str = next.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                          println!("Status: {}\nAllowed transitions: {}", spec.status.as_str(), next_str);
                      }
                  }
              }
          }
'''

new_lines = lines[:insert_idx] + [spec_handler] + lines[insert_idx:]

with open('src/main.rs', 'w') as f:
    f.writelines(new_lines)

print("Inserted Spec CLI handler at line", insert_idx + 1)
