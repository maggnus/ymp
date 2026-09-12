use serde_json::json;
use std::{fs, path::Path, time::Instant};
use ymp_core::{new_id, now, MemoryEntry};
use ymp_storage::Store;
use ymp_workspace::Workspace;

fn names(root: &Path) -> Vec<std::path::PathBuf> {
    let mut paths=walkdir::WalkDir::new(root).follow_links(false).into_iter()
        .filter_entry(|e| e.depth()==0 || ![".git","node_modules","target","__pycache__",".DS_Store",".ymp2"].contains(&e.file_name().to_string_lossy().as_ref()))
        .map(|e|e.unwrap()).filter(|e| e.file_type().is_file() || e.file_type().is_symlink()).map(|e|e.path().to_owned()).collect::<Vec<_>>();
    paths.sort();paths
}
fn measure(root: &Path, metadata: &Path) -> serde_json::Value {
    let workspace=Workspace::open(root,metadata).unwrap();
    let start=Instant::now(); let mut paths=workspace.files().unwrap(); let hashed=start.elapsed().as_secs_f64();
    let start=Instant::now(); let candidate=names(&workspace.directory); let listed=start.elapsed().as_secs_f64();
    paths.sort(); assert_eq!(paths,candidate);
    let full=serde_json::to_string(&paths).unwrap();
    let bounded=serde_json::to_string(&paths.iter().take(200).collect::<Vec<_>>()).unwrap();
    let target=paths.iter().find(|p|p.ends_with("zzz-required.md"));
    let mut targeted=Vec::new();
    if let Some(target)=target {targeted.push(target);}
    targeted.extend(paths.iter().filter(|p|Some(*p)!=target).take(199));
    let selected=serde_json::to_string(&targeted).unwrap();
    json!({"files":paths.len(),"regular_file_bytes":paths.iter().filter(|p|!p.is_symlink()).map(|p|p.metadata().unwrap().len()).sum::<u64>(),
           "absolute_listing_bytes":full.len(),"first_200_bytes":bounded.len(),"targeted_200_bytes":selected.len(),
           "full_contains_target":full.contains("zzz-required.md"),"first_200_contains_target":bounded.contains("zzz-required.md"),"targeted_contains_target":selected.contains("zzz-required.md"),
           "fingerprint_seconds":hashed,"names_only_seconds":listed,"path_sets_equal":true})
}
fn main() {
    let scratch=tempfile::Builder::new().prefix("yc-").tempdir_in("/tmp").unwrap();
    let mut listings=Vec::new();
    for n in [10,1000,10000] {
        let root=scratch.path().join(format!("files-{n}"));fs::create_dir(&root).unwrap();
        for i in 0..n {fs::write(root.join(format!("document-{i:05}.md")),vec![b'x';1024]).unwrap();}
        fs::write(root.join("zzz-required.md"),"The requested artifact.\n").unwrap();
        listings.push(json!({"fixture":format!("{n} small files"),"measurement":measure(&root,&scratch.path().join(format!("meta-{n}")))}));
    }
    let root=scratch.path().join("large-file");fs::create_dir(&root).unwrap();
    fs::write(root.join("large.bin"),vec![b'x';64*1024*1024]).unwrap();
    fs::write(root.join("zzz-required.md"),"The requested artifact.\n").unwrap();
    listings.push(json!({"fixture":"64 MiB regular file","measurement":measure(&root,&scratch.path().join("large-meta"))}));
    if let Some(project)=std::env::args().nth(1) {
        listings.push(json!({"fixture":"ymp working tree during research, including untracked artifacts","measurement":measure(Path::new(&project),&scratch.path().join("repo-meta"))}));
    }
    let store=Store::open(&scratch.path().join("memory")).unwrap();
    let project=store.project(scratch.path()).unwrap();
    for (id,title,content) in [
        ("relevant","CSV normalization","Use Decimal to normalize invoice totals; preserve duplicate order."),
        ("generic","Work preparation","Plan future execution with write permissions. Inspect files before changing them."),
    ] {
        store.save_memory(&MemoryEntry{id:id.into(),project_id:Some(project.id.clone()),kind:"procedure".into(),title:title.into(),content:content.into(),source_session:new_id(),author:"writer".into(),reviewer:Some("reviewer".into()),status:"active".into(),created_at:now(),supersedes:None}).unwrap();
    }
    let prefix="Bid for a future execution turn with write permissions. This bidding turn is read-only; that is not a reason to decline. Decide whether your capabilities fit this task: Normalize invoice totals and preserve duplicate order.";
    let focused="Normalize invoice totals preserve duplicate order";
    let original:Vec<_>=store.memory(Some(&project.id),prefix).unwrap().iter().map(|m|m.id.clone()).collect();
    let revised:Vec<_>=store.memory(Some(&project.id),focused).unwrap().iter().map(|m|m.id.clone()).collect();
    assert!(!original.contains(&"relevant".to_owned()));assert!(revised.contains(&"relevant".to_owned()));
    let mut messages=vec!["MID_RUN_REQUIREMENT: preserve the existing author credits".to_owned()];
    messages.extend((0..13).map(|i|format!("Coordination message {i}")));
    let suffix=messages.iter().rev().take(12).cloned().collect::<Vec<_>>().join("\n");
    assert!(!suffix.contains("MID_RUN_REQUIREMENT"));
    println!("{}",json!({"listings":listings,"memory":{"boilerplate_prefix_query_returns":original,"task_query_returns":revised},
        "context":{"messages":messages.len(),"last_12_retains_mid_run_requirement":false,"original_request_is_separately_injected_into_reviews":true},
        "caveat":"Offline availability and performance checks, not model-quality A/B evidence. Timings are single warm-cache observations."}));
}
