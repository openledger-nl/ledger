//! The DOT framework (Development Oriented Triangulation, HBO-i's ICT Research Methods, https://ictresearchmethods.nl):
//! which research methods a document is made with, and why.
//!
//! A deliverable keeps its plan in its own frontmatter, so it travels with the file and every published version keeps
//! the plan it was made with:
//!
//! ```yaml
//! dot:
//!   - method: Literature study
//!     why: What is known about syncing plain files
//!   - method: Interview
//! ```
//!
//! A line of its own reading `!dotmethod` shows the plan as a table (see `markdown::render_document`). A method that
//! is not one of the cards keeps the strategy it was given (`strategy: lab`).

use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

use crate::error::{Error, Result};
use crate::frontmatter::{self, MetaExt};
use crate::markdown;

/// The frontmatter key the plan is kept under.
const KEY: &str = "dot";
/// What a line says to show the plan as a table.
pub const TABLE_COMMAND: &str = "!dotmethod";

/// The five research strategies, and the Joker's "extra".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    Library,
    Field,
    Lab,
    Showroom,
    Workshop,
    Extra,
}

use Strategy::*;

impl Strategy {
    pub fn label(self) -> &'static str {
        match self {
            Library => "Library",
            Field => "Field",
            Lab => "Lab",
            Showroom => "Showroom",
            Workshop => "Workshop",
            Extra => "Extra",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Library => "library",
            Field => "field",
            Lab => "lab",
            Showroom => "showroom",
            Workshop => "workshop",
            Extra => "extra",
        }
    }

    fn parse(text: &str) -> Option<Strategy> {
        [Library, Field, Lab, Showroom, Workshop, Extra].into_iter().find(|s| s.id().eq_ignore_ascii_case(text.trim()))
    }
}

/// One of the method cards.
#[derive(Debug, Clone, Serialize)]
pub struct DotMethod {
    pub name: &'static str,
    pub strategy: Strategy,
    /// The card's own "Why?".
    pub why: &'static str,
}

const fn m(name: &'static str, strategy: Strategy, why: &'static str) -> DotMethod {
    DotMethod { name, strategy, why }
}

/// Every card of the ICT Research Methods pack (HBO-i, 2018), by strategy.
pub static CATALOG: &[DotMethod] = &[
    m("Available product analysis", Library, "Find out if what you are planning to do has already been done (in full or in part) by someone else."),
    m("Best good and bad practices", Library, "Incorporating what has proven to work somewhere else forms the basis of any high-quality project."),
    m("Community research", Library, "Have others already tackled the problem? Incorporating your peers’ knowledge can seriously speed things up."),
    m("Competitive analysis", Library, "Find a niche or unique selling point competitors are not filling."),
    m("Design pattern research", Library, "Applying well-known design patterns improves the quality (and structure) of the designed software."),
    m("Expert interview", Library, "A domain expert can put you on the right track when you enter a new domain or field of expertise. The expert can recommend sources, give you a sense of direction or point out common pitfalls."),
    m("Literature study", Library, "Find general information, guidance and best practices."),
    m("SWOT analysis", Library, "A SWOT analysis supports decision makers in identifying relevant factors for achieving certain goals."),
    m("Descriptive statistics", Field, "To get an overview and summary of a dataset."),
    m("Document analysis", Field, "Documentation produced by the company can be a great first resource for understanding the organisation you are working for and their work processes."),
    m("Domain modelling", Field, "Map the domain that your product will be part of, so you know the key concepts and the relations between them."),
    m("Exploratory data analysis (ML)", Field, "Find something interesting in the data, check understanding of the domain or problem space, generate new questions based on the data."),
    m("Explore user requirements", Field, "Get a detailed view of how users will be using your solution and what their requirements are."),
    m("Focus group", Field, "A focus group discussion is an efficient way to gain insight into how people think about an issue, without having to interview each person separately."),
    m("Human impact score", Field, "If you create technology you change people and you change how people look at the world. You make moral decisions. Researching how your technology changes people and designing for that change improves the chance for a technology with the intended (positive) impact and with that the acceptance of the technology as a whole"),
    m("Interview", Field, "Learn from potential users of your new product and other stakeholders"),
    m("Observation", Field, "Get a feeling for how your intended users will use your product by unobtrusively observing them in their natural environment, doing the things they always do."),
    m("Problem analysis", Field, "Before solving a problem, it is important to understand it. Moreover, problem analysis ensures that you are not solving the wrong problem."),
    m("Stakeholder analysis", Field, "Identify the stakeholders and ensure that their needs are considered."),
    m("Survey", Field, "Collect information (mostly quantitative) from a large sample of your target group"),
    m("Task analysis", Field, "Understand the structure, flow or other aspects of a certain task. Task analysis focuses on what end users actually do to achieve their goals."),
    m("A/B testing", Lab, "A minor change in a design may alter user behaviour in ways that are hard to detect in a usability test. An A/B test allows you to compare real-world user behaviour across different versions of a product."),
    m("Component test", Lab, "Test a subsystem or component in isolation to ensure its correctness before integrating it with other components or modules."),
    m("Computer simulation", Lab, "Computer simulations are used when a real-world process, system or event situation is not available or feasible."),
    m("Data analytics", Lab, "Gain insights by measuring and analysing data. Researching a dataset can give you useful quantitative information about the topic of interest."),
    m("Data quality check (ML)", Lab, "Ensure that the data you are using is of sufficient quality to base further conclusions on."),
    m("Hardware validation", Lab, "Hardware does not always perform according to its specifications. Hardware validation ensures that the hardware performs as expected and excludes hardware as a source of errors."),
    m("Inferential statistics", Lab, "To test hypothesis based on a quantitative dataset."),
    m("Model evaluation (ML)", Lab, "Verify the correctness and usefulness of the results of your model with the stakeholders or compare different models with respect to their usefulness."),
    m("Model validation (ML)", Lab, "Ensure that your model produces results of sufficient quality to base your conclusions on."),
    m("Non-functional test", Lab, "Test the operating conditions under which the system delivers its intended functions."),
    m("Security test", Lab, "Understand the security risks of an IT system."),
    m("System test", Lab, "Solve problems before your system goes into production and demonstrate that the system operates according to its requirements."),
    m("Unit test", Lab, "Find certain types of bugs as early as possible and ensure your code keeps running after a change."),
    m("Usability testing", Lab, "Detect problems users have with your solution and correct them before the system goes live."),
    m("Benchmark test", Showroom, "A standardised set allows you to compare your product to similar products."),
    m("Ethical check", Showroom, "Norms and values differ between various people and societies. Make sure your design and development decisions do not lead to conflicts with certain norms and values."),
    m("Guideline conformity analysis", Showroom, "Conforming to guidelines and standards helps ensure the credibility of the quality of your product and prevents reliability, privacy and security issues."),
    m("Peer review", Showroom, "Colleagues and experts can help improve your work, especially if they need to reuse it."),
    m("Pitch", Showroom, "Get an idea of the unique selling points of the opportunity you have found, or of the idea you have to tackle a problem, and practice concise communication about them."),
    m("Product review", Showroom, "Ensure that the product is perfect before it is released to the client or users."),
    m("Static program analysis", Showroom, "Get an impression of how well your code is written and quickly find vulnerabilities, weak spots and bad smells."),
    m("Brainstorm", Workshop, "Generate and develop new ideas."),
    m("Business case exploration", Workshop, "Maximize the outcome of your efforts or investment."),
    m("Co-creation", Workshop, "To gain inspiration from your users by involving them in the development process."),
    m("Code review", Workshop, "With many eyeballs on the code, all bugs are shallow. Colleagues can help you find bugs and improve the quality of your source code."),
    m("Decomposition", Workshop, "Breaking a complex IT system or problem into smaller parts ensures its maintainability and robustness, and facilitates cooperation in large-scale software projects."),
    m("Gap analysis", Workshop, "Compare an actual situation to an ideal or desired situation. The gap analysis can be used to create plans to bridge the gaps."),
    m("Hackathon", Workshop, "To find new solutions, be able to try them out quickly and involve many people in your challenge."),
    m("IT architecture sketching", Workshop, "Defining the IT architecture is complex and requires contributions from and interactions between software designers and/or architects. Sketching facilitates these discussions."),
    m("Multi-criteria decision making", Workshop, "Improve the quality of complex decisions"),
    m("Prototyping", Workshop, "Develop, evaluate or communicate a concept, design or problem solution to make your ideas concrete, to learn whether they work and to discover the technical limitations or possibilities."),
    m("Requirements prioritization", Workshop, "To define and weigh the requirements of a new design or redesign, considering the interest of all concerned stakeholders."),
    m("Root cause analysis", Workshop, "Understand why a problem occurs and prevent it from happening again."),
    m("Silicon sampling", Workshop, "Understanding how technology changes people is difficult — many effects are unexpected and not immediately visible. Large Language Models (LLMs) can serve as a tool to broaden perspectives, using their knowledge of cultural, social, and historical patterns found online. By sampling diverse voices and situations through LLM prompts, participants can surface both likely and unlikely consequences of a technology. These insights feed directly into design choices and support more responsible innovation."),
    m("Joker", Extra, "This set of cards gives you an overview of methods you can use, but there are many others that might fit your goals better."),
];

/// The card called `name`, whatever its capitals.
pub fn find(name: &str) -> Option<&'static DotMethod> {
    let name = name.trim();
    CATALOG.iter().find(|m| m.name.eq_ignore_ascii_case(name))
}

/// A method chosen for a document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlannedMethod {
    pub method: String,
    /// The card's strategy, or the one given to a method of your own; none when neither says.
    pub strategy: Option<Strategy>,
    /// Why this document uses it, in your words.
    pub why: String,
}

/// The plan in a file's frontmatter, with each card's strategy filled in. Lenient, as frontmatter is read everywhere:
/// an item may be just the method's name (`- Interview`).
pub fn plan_of(meta: &Mapping) -> Vec<PlannedMethod> {
    let Some(Value::Sequence(items)) = meta.get(KEY) else { return vec![] };
    items
        .iter()
        .filter_map(|item| {
            let (method, strategy, why) = match item {
                Value::Mapping(map) => (map.text("method")?, map.text("strategy"), map.text("why").unwrap_or_default()),
                Value::String(name) if !name.trim().is_empty() => (name.clone(), None, String::new()),
                _ => return None,
            };
            let strategy = find(&method).map(|card| card.strategy).or_else(|| strategy.as_deref().and_then(Strategy::parse));
            Some(PlannedMethod { method: method.trim().to_string(), strategy, why: why.trim().to_string() })
        })
        .collect()
}

/// The plan in a file's text.
pub fn plan_in(text: &str) -> Vec<PlannedMethod> {
    frontmatter::parse(text).map(|p| plan_of(&p.meta)).unwrap_or_default()
}

/// The file with `plan` as its plan, the rest of its frontmatter and its body as they were. An empty plan takes the
/// key away (and a frontmatter block that only held it).
pub fn with_plan(text: &str, plan: &[PlannedMethod]) -> Result<String> {
    let (mut meta, body) = match frontmatter::parse(text) {
        Some(parsed) => (parsed.meta, parsed.body),
        None if text.trim_start_matches('\u{feff}').starts_with("---") => {
            return Err(Error::invalid("The frontmatter at the top of this file can't be read: fix it in Source mode first"));
        }
        None => (Mapping::new(), text.to_string()),
    };
    meta.remove(KEY);
    let items: Vec<Value> = plan
        .iter()
        .filter(|p| !p.method.trim().is_empty())
        .map(|p| {
            let mut item = Mapping::new();
            frontmatter::set(&mut item, "method", p.method.trim());
            // A card's strategy is the card's; only a method of your own needs it written down.
            if let (None, Some(strategy)) = (find(&p.method), p.strategy) {
                frontmatter::set(&mut item, "strategy", strategy.id());
            }
            frontmatter::set_opt(&mut item, "why", Some(p.why.trim()));
            Value::Mapping(item)
        })
        .collect();
    if !items.is_empty() {
        frontmatter::set(&mut meta, KEY, Value::Sequence(items));
    }
    if meta.is_empty() {
        let mut out = body.trim_end().to_string();
        out.push('\n');
        return Ok(out);
    }
    Ok(frontmatter::render(&meta, &body))
}

/// What the page needs to draw `!dotmethod` while it is being typed, and to count the methods.
#[derive(Debug, Clone, Serialize)]
pub struct DotTable {
    pub html: String,
    pub count: usize,
}

pub fn table_of(text: &str) -> DotTable {
    let plan = plan_in(text);
    DotTable { html: table_html(&plan), count: plan.len() }
}

/// The plan as the table `!dotmethod` shows: strategy, method and, once any is written, why. Without a plan, a line
/// saying how to make one.
pub fn table_html(plan: &[PlannedMethod]) -> String {
    if plan.is_empty() {
        return "<p class=\"dot-empty\">No research methods chosen for this document yet. Choose them under <strong>Research methods</strong>.</p>\n".into();
    }
    let with_why = plan.iter().any(|p| !p.why.is_empty());
    let mut out = String::from("<table class=\"dot-plan\">\n<thead><tr><th>Strategy</th><th>Method</th>");
    if with_why {
        out.push_str("<th>Why</th>");
    }
    out.push_str("</tr></thead>\n<tbody>\n");
    for p in plan {
        out.push_str("<tr><td>");
        if let Some(strategy) = p.strategy.or_else(|| find(&p.method).map(|card| card.strategy)) {
            out.push_str(&format!("<span class=\"dot-strategy dot-{}\">{}</span>", strategy.id(), strategy.label()));
        }
        out.push_str("</td><td");
        if let Some(card) = find(&p.method) {
            out.push_str(&format!(" title=\"{}\"", escape(card.why)));
        }
        out.push_str(&format!(">{}</td>", escape(&p.method)));
        if with_why {
            out.push_str(&format!("<td>{}</td>", inline(&p.why)));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody>\n</table>\n");
    out
}

/// A why as markdown (so a `[[link]]` or some emphasis in it works), without the paragraph around a single one.
fn inline(why: &str) -> String {
    let html = markdown::render(why);
    let trimmed = html.trim_end();
    match trimmed.strip_prefix("<p>").and_then(|rest| rest.strip_suffix("</p>")) {
        Some(inner) if !inner.contains("<p>") => inner.to_string(),
        _ => html,
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planned(method: &str, why: &str) -> PlannedMethod {
        PlannedMethod { method: method.into(), strategy: None, why: why.into() }
    }

    #[test]
    fn the_catalog_holds_every_card_once() {
        assert_eq!(CATALOG.len(), 56);
        for card in CATALOG {
            assert_eq!(CATALOG.iter().filter(|c| c.name.eq_ignore_ascii_case(card.name)).count(), 1, "{}", card.name);
        }
        assert_eq!(find("literature STUDY").map(|c| c.strategy), Some(Library));
    }

    #[test]
    fn a_plan_is_written_into_the_frontmatter_and_read_back() {
        let text = "---\nid: abc\n---\n\n# Report\n\n!dotmethod\n";
        let plan = vec![planned("Literature study", "What is known"), PlannedMethod { method: "Diary study".into(), strategy: Some(Field), why: String::new() }];
        let written = with_plan(text, &plan).unwrap();
        assert!(written.starts_with("---\nid: abc\ndot:\n"), "{written}");
        assert!(written.ends_with("# Report\n\n!dotmethod\n"), "{written}");
        let read = plan_in(&written);
        assert_eq!(read[0], PlannedMethod { method: "Literature study".into(), strategy: Some(Library), why: "What is known".into() });
        assert_eq!(read[1].strategy, Some(Field));
        // A card's strategy is not written down; a method of your own's is.
        assert!(!written.contains("strategy: library") && written.contains("strategy: field"), "{written}");
    }

    #[test]
    fn a_file_without_frontmatter_gets_one_and_loses_it_again() {
        let text = "# Report\n\nText.\n";
        let written = with_plan(text, &[planned("Interview", "")]).unwrap();
        assert_eq!(written, "---\ndot:\n- method: Interview\n---\n\n# Report\n\nText.\n");
        assert_eq!(with_plan(&written, &[]).unwrap(), text);
    }

    #[test]
    fn broken_frontmatter_is_not_overwritten() {
        assert!(with_plan("---\n: : nope\n---\n\nText", &[planned("Interview", "")]).is_err());
    }

    #[test]
    fn a_bare_name_reads_as_a_method() {
        let plan = plan_in("---\ndot:\n- Survey\n- method: Hunch\n  strategy: Workshop\n---\n");
        assert_eq!(plan.iter().map(|p| (p.method.as_str(), p.strategy)).collect::<Vec<_>>(), [("Survey", Some(Field)), ("Hunch", Some(Workshop))]);
    }

    #[test]
    fn the_table_has_a_why_column_only_when_a_why_is_written() {
        assert!(!table_html(&[planned("Survey", "")]).contains("<th>Why</th>"));
        let html = table_html(&[planned("Survey", "Ask *many* people"), planned("<b>x</b>", "")]);
        assert!(html.contains("<th>Why</th>") && html.contains("<td>Ask <em>many</em> people</td>"), "{html}");
        assert!(html.contains("dot-strategy dot-field") && html.contains("&lt;b&gt;x&lt;/b&gt;"), "{html}");
        assert!(table_html(&[]).contains("dot-empty"));
    }
}
