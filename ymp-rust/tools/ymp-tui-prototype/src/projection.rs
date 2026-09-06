//! Read-only projections. Archiving changes visibility, never the identity or content of evidence.
use crate::{
    entities::*,
    resources::{Counts, Dataset, Kind, Record, World},
    simulation::{Simulation, age},
};
use std::collections::BTreeMap;
impl Simulation {
    pub fn project(&self) -> World {
        let selected = self.task().map(|t| t.id.as_str());
        let relevant = |id: &str| selected == Some(id);
        let scoped: Vec<_> = self.agents.iter().filter(|a| relevant(&a.task)).collect();
        let counts = Counts {
            running: scoped
                .iter()
                .filter(|a| !a.archived && matches!(a.state, Status::Starting | Status::Running))
                .count(),
            yielded: scoped
                .iter()
                .filter(|a| !a.archived && a.state == Status::Waiting)
                .count(),
            failed: scoped
                .iter()
                .filter(|a| !a.archived && matches!(a.state, Status::Failed | Status::Interrupted))
                .count(),
            completed: scoped
                .iter()
                .filter(|a| !a.archived && a.state == Status::Completed)
                .count(),
        };
        let mut models = BTreeMap::new();
        for a in scoped.iter().filter(|a| !a.archived) {
            *models.entry(a.profile.model.clone()).or_insert(0) += 1;
        }
        let agents=Dataset::new(Kind::Agents,vec!["AGENT","STATE","MODEL","EFFORT","AGE","TASK"],vec![10,12,19,7,7],scoped.into_iter().map(|a|{
            let mut row=Record::new(a.id.clone(),if a.archived{"Archived"}else{a.state.label()},vec![a.id.clone(),if a.archived{"Archived"}else{a.state.label()}.into(),a.profile.model.clone(),a.profile.effort.clone(),age(self.now,a.created),a.task.clone()],
                format!("State: {}\nCreated by: {}\nConnection: {} ({})\nProvider: {}\nRuntime: {}\nModel: {}\nEffort: {}\nRun: {}\nLifecycle duration: {}\nReplaces: {}\n\nAssignment\n{}\n\nOutput\n{}",a.state.label(),a.source,a.profile.connection_name,a.profile.connection,a.profile.provider,a.profile.engine,a.profile.model,a.profile.effort,if a.run==0{"Clarification".into()}else{format!("R-{:03}",a.run)},age(a.ended.unwrap_or(self.now),a.created),a.replaces.as_deref().unwrap_or("—"),a.assignment,if a.output.is_empty(){"No output returned"}else{&a.output}))
                .archived(a.archived).link('t',"Task",Kind::Tasks,format!("id={}",a.task)).link('b',"Messages",Kind::Board,format!("author={}",a.id)).link('l',"Tools",Kind::Tools,format!("agent={}",a.id)).link('h',"History",Kind::Activity,format!("entity={}",a.id));
            if self.providers.iter().any(|p|p.id==a.profile.connection){row=row.link('p',"Connection",Kind::Providers,format!("id={}",a.profile.connection));}
            row
        }).collect());
        let tasks=Dataset::new(Kind::Tasks,vec!["TASK","STATE","AGE","REQUEST"],vec![10,19,8],self.tasks.iter().map(|t|{
            let profiles=if t.profiles.is_empty(){self.profiles()}else{t.profiles.clone()};
            let routes=profiles.iter().map(|p|format!("{} · {} · {} · {}",p.connection_name,p.engine,p.model,p.effort)).collect::<Vec<_>>().join("\n");
            let runs=self.runs.iter().filter(|r|r.task==t.id).map(|r|format!("R-{:03} · {} · {} · {} agents / {} parallel",r.number,r.state.label(),age(r.ended.unwrap_or(self.now),r.started),r.max_agents,r.max_parallel)).collect::<Vec<_>>().join("\n");
            Record::new(t.id.clone(),if t.archived{"Archived"}else{t.stage.label()},vec![t.id.clone(),if t.archived{"Archived"}else{t.stage.label()}.into(),age(self.now,t.created),t.goal.clone()],
                format!("State: {}\nRequest: {}\nRequirements: {}\n\nRuns\n{}\n\nModel access\n{}\n\nCandidate: {}\n\nA frozen run retains its own specification and settings.",t.stage.label(),t.goal,t.requirements,if runs.is_empty(){"Not started"}else{&runs},if routes.is_empty(){"No enabled connection"}else{&routes},if t.candidate==0{"—".into()}else{format!("R-{:03}/C-{:03}",t.run,t.candidate)}))
                .archived(t.archived).link('a',"Agents",Kind::Agents,format!("task={}",t.id)).link('b',"Board",Kind::Board,String::new()).link('v',"Checks",Kind::Checks,String::new()).link('f',"Files",Kind::Files,String::new()).link('h',"History",Kind::Activity,format!("entity={}",t.id))
        }).collect());
        let knowledge=Dataset::new(Kind::Board,vec!["MESSAGE","AUTHOR","AGE","CONTENT"],vec![10,10,8],self.posts.iter().filter(|p|relevant(&p.task)).rev().map(|p|Record::new(p.id.clone(),"Message",vec![p.id.clone(),p.author.clone(),age(self.now,p.at),p.text.clone()],format!("Author: {}\nTask: {}\n\n{}\n\nAttributed statement; not a verification verdict.",p.author,p.task,p.text)).link('a',"Author",Kind::Agents,format!("id={}",p.author))).collect());
        let providers=Dataset::new(Kind::Providers,vec!["NAME","TYPE","ENABLED","STATE","CHECKED"],vec![22,12,9,15],self.providers.iter().map(|p|{
            let status=if !p.enabled{"Disabled"}else if p.checked.is_none(){"Not checked"}else{"Demo ready"};
            Record::new(p.id.clone(),status,vec![p.name.clone(),p.family.name().into(),if p.enabled{"Yes"}else{"No"}.into(),status.into(),p.checked.map_or("—".into(),|at|age(self.now,at))],format!("Connection: {}\nName: {}\nType: {}\nRuntime: {}\nModel: {}\nEffort: low\n\nEnabled: {}\nProfile observation: {}\n\nThe observation is simulated. Configuration affects future runs; existing agents retain their frozen connection and profile.",p.id,p.name,p.family.name(),p.family.engine(),p.family.model(),p.enabled,p.checked.map_or("Not checked".into(),|at|age(self.now,at)))).link('h',"History",Kind::Activity,format!("entity={}",p.id))
        }).collect());
        let tools=Dataset::new(Kind::Tools,vec!["TOOL","STATE","AGENT","AGE","COMMAND"],vec![9,11,10,7],self.tools.iter().filter(|t|relevant(&t.task)).map(|t|Record::new(t.id.clone(),t.status.label(),vec![t.id.clone(),t.status.label().into(),t.agent.clone(),age(self.now,t.started),t.command.clone()],format!("Task: {}\nRun: R-{:03}\nOwner: {}\nWorkspace: isolated candidate (simulated)\nDuration: {}\n\n{}\n\n{}",t.task,t.run,t.agent,age(t.ended.unwrap_or(self.now),t.started),t.command,t.output)).link('a',"Agent",Kind::Agents,format!("id={}",t.agent))).collect());
        let checks = Dataset::new(
            Kind::Checks,
            vec!["CHECK", "RESULT", "CANDIDATE", "NAME"],
            vec![9, 11, 14],
            self.checks
                .iter()
                .filter(|c| relevant(&c.task))
                .map(|c| {
                    Record::new(
                        c.id.clone(),
                        if c.passed { "Passed" } else { "Failed" },
                        vec![
                            c.id.clone(),
                            if c.passed { "Passed" } else { "Failed" }.into(),
                            format!("R-{:03}/C-{:03}", c.run, c.candidate),
                            c.name.clone(),
                        ],
                        c.detail.clone(),
                    )
                })
                .collect(),
        );
        let mut files = Vec::new();
        for c in self.candidates.iter().filter(|c| relevant(&c.task)) {
            for path in &c.files {
                files.push(Record::new(format!("{}/{}",c.id,path),"Candidate",vec![path.clone(),format!("R-{:03}/C-{:03}",c.run,c.number),"Simulated".into()],format!("Candidate: {}\nCreated: {} ago\nFile: {}\n\nImmutable simulated manifest. This is not a generated game file on disk. Export writes a report describing this candidate, not playable source code.",c.id,age(self.now,c.created),path)));
            }
        }
        for e in self.exports.iter().filter(|e| relevant(&e.task)) {
            files.push(Record::new(e.id.clone(),if e.removed{"Removed"}else{"Exported"},vec![e.relative_path.clone(),e.entity.clone(),if e.removed{"Removed"}else{"Exported report"}.into()],format!("Export: {}\nSubject: {} {}\nPath: {}\nState: {}\n\nOnly this application-owned report can be removed. Evidence and source records remain.",e.id,e.kind,e.entity,e.relative_path,if e.removed{"Removed"}else{"Present"})).archived(e.removed));
        }
        let files = Dataset::new(
            Kind::Files,
            vec!["FILE", "SUBJECT", "STATE"],
            vec![31, 18],
            files,
        );
        let activity = Dataset::new(
            Kind::Activity,
            vec!["EVENT", "ACTOR", "ACTION", "ENTITY", "AGE"],
            vec![9, 18, 26, 18],
            self.audit
                .iter()
                .rev()
                .map(|a| {
                    Record::new(
                        a.sequence.to_string(),
                        "Event",
                        vec![
                            a.sequence.to_string(),
                            a.actor.clone(),
                            a.action.clone(),
                            a.entity.clone(),
                            age(self.now, a.at),
                        ],
                        a.detail.clone(),
                    )
                })
                .collect(),
        );
        World {
            agents,
            tasks,
            knowledge,
            providers,
            tools,
            checks,
            files,
            activity,
            sessions: Dataset::new(Kind::Sessions, vec!["SESSION"], vec![], vec![]),
            counts,
            models: models.into_iter().collect(),
        }
    }
}
