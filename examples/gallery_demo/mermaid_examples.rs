#[derive(Clone, Copy)]
pub(crate) struct MermaidExample {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) source: &'static str,
}

pub(crate) const EXAMPLES: &[MermaidExample] = &[
    MermaidExample {
        id: "architecture",
        label: "Architecture",
        source: "architecture-beta\n    group platform(cloud)[Platform]\n    service api(server)[API] in platform\n    service db(database)[Database] in platform\n    api:R -- L:db",
    },
    MermaidExample {
        id: "block",
        label: "Block layout",
        source: "block\n    ingest[\"Ingest\"] --> transform[\"Transform\"]\n    transform --> publish[\"Publish\"]",
    },
    MermaidExample {
        id: "c4",
        label: "C4 context",
        source: "C4Context\n    title Checkout context\n    Person(customer, \"Customer\")\n    System(checkout, \"Checkout\", \"Places orders\")\n    System_Ext(payments, \"Payments\", \"Charges cards\")\n    Rel(customer, checkout, \"Uses\")\n    Rel(checkout, payments, \"Charges\")",
    },
    MermaidExample {
        id: "class",
        label: "Class diagram",
        source: "classDiagram\n    class Order {\n      +String id\n      +Money total\n      +submit()\n    }\n    class LineItem {\n      +String sku\n      +int quantity\n    }\n    Order \"1\" *-- \"many\" LineItem",
    },
    MermaidExample {
        id: "er",
        label: "Entity relationship",
        source: "erDiagram\n    CUSTOMER ||--o{ ORDER : places\n    ORDER ||--|{ LINE_ITEM : contains\n    CUSTOMER {\n      string id PK\n      string email\n    }\n    ORDER {\n      string id PK\n      string customer_id FK\n    }",
    },
    MermaidExample {
        id: "flowchart",
        label: "Release flow",
        source: "flowchart LR\n    A[Draft] --> B{Review}\n    B -->|Approved| C[Release]\n    B -->|Changes requested| A\n    C --> D[(Metrics)]",
    },
    MermaidExample {
        id: "gantt",
        label: "Gantt plan",
        source: "gantt\n    title Release plan\n    dateFormat YYYY-MM-DD\n    section Build\n    Design :done, design, 2026-01-01, 3d\n    Implement :active, impl, after design, 5d\n    section Ship\n    Verify :verify, after impl, 2d\n    Release :milestone, after verify, 0d",
    },
    MermaidExample {
        id: "gitgraph",
        label: "Git graph",
        source: "gitGraph:\n    commit id: \"bootstrap\"\n    branch feature\n    checkout feature\n    commit id: \"add diagram\"\n    checkout main\n    merge feature\n    commit id: \"release\"",
    },
    MermaidExample {
        id: "info",
        label: "Info",
        source: "info showInfo",
    },
    MermaidExample {
        id: "journey",
        label: "User journey",
        source: "journey\n    title Checkout journey\n    section Browse\n      Find product: 5: Customer\n      Add to cart: 4: Customer\n    section Pay\n      Enter card: 2: Customer\n      Receive receipt: 5: Customer, Service",
    },
    MermaidExample {
        id: "kanban",
        label: "Kanban",
        source: "kanban\n    Todo\n      audit[Audit access]\n      docs[Write docs]\n    Doing\n      render[Render preview]\n    Done\n      ship[Ship release]",
    },
    MermaidExample {
        id: "mindmap",
        label: "Mindmap",
        source: "mindmap\n    product[Product]\n      experience\n        onboarding\n        search\n      platform\n        api\n        observability",
    },
    MermaidExample {
        id: "packet",
        label: "Packet",
        source: "packet\n    +8: \"Version\"\n    +8: \"Flags\"\n    +16: \"Length\"\n    +32: \"Request ID\"",
    },
    MermaidExample {
        id: "pie",
        label: "Pie chart",
        source: "pie title Deployment share\n    \"Stable\" : 62\n    \"Canary\" : 23\n    \"Preview\" : 15",
    },
    MermaidExample {
        id: "quadrantchart",
        label: "Quadrant chart",
        source: "quadrantChart\n    title Work prioritization\n    x-axis Low impact --> High impact\n    y-axis Low effort --> High effort\n    quadrant-1 Plan\n    quadrant-2 Invest\n    quadrant-3 Skip\n    quadrant-4 Delegate\n    Observability: [0.8, 0.7]\n    Documentation: [0.6, 0.3]",
    },
    MermaidExample {
        id: "radar",
        label: "Radar chart",
        source: "radar-beta\n    title Platform scorecard\n    axis Reliability, Performance, Security, DX, Cost\n    curve Current{8,7,9,6,7}\n    curve Target{9,9,9,8,8}",
    },
    MermaidExample {
        id: "requirement",
        label: "Requirement",
        source: "requirementDiagram\n    requirement availability {\n      id: REQ-1\n      text: Service stays available\n      risk: high\n      verifymethod: test\n    }\n    element api {\n      type: service\n    }\n    api - satisfies -> availability",
    },
    MermaidExample {
        id: "sankey",
        label: "Sankey",
        source: "sankey\n    Ingest,Validate,100\n    Validate,Process,82\n    Validate,Reject,18\n    Process,Publish,74\n    Process,Retry,8",
    },
    MermaidExample {
        id: "sequence",
        label: "Request sequence",
        source: "sequenceDiagram\n    participant UI\n    participant API\n    participant Store\n    UI->>API: submit request\n    API->>Store: save record\n    Store-->>API: record id\n    API-->>UI: success",
    },
    MermaidExample {
        id: "state",
        label: "Service state",
        source: "stateDiagram-v2\n    [*] --> Starting\n    Starting --> Running\n    Running --> Degraded: health check fails\n    Degraded --> Running: recovered\n    Running --> Stopped\n    Stopped --> [*]",
    },
    MermaidExample {
        id: "timeline",
        label: "Timeline",
        source: "timeline\n    title Product timeline\n    2024 : Foundation\n         : First customers\n    2025 : Automation\n         : Global launch\n    2026 : Intelligence\n         : Agent workflows",
    },
    MermaidExample {
        id: "treemap",
        label: "Treemap",
        source: "treemap\n    \"Engineering\": 70\n      \"Platform\": 35\n      \"Product\": 35\n    \"Operations\": 30\n      \"Support\": 18\n      \"Security\": 12",
    },
    MermaidExample {
        id: "venn",
        label: "Venn",
        source: "venn-beta\n    title Capability overlap\n    set A[Engineering]:20\n    set B[Operations]:16\n    set C[Product]:14\n    union A,B[Reliability]:5\n    union A,C[Delivery]:4\n    union B,C[Feedback]:3\n    union A,B,C[Customer value]:1",
    },
    MermaidExample {
        id: "xychart",
        label: "XY chart",
        source: "xychart-beta\n    title \"Weekly requests\"\n    x-axis [Mon, Tue, Wed, Thu, Fri]\n    y-axis \"Requests\" 0 --> 100\n    bar [45, 67, 72, 61, 88]\n    line [40, 54, 68, 65, 80]",
    },
    MermaidExample {
        id: "zenuml",
        label: "ZenUML",
        source: "zenuml\n    Client->Gateway: request\n    Gateway->Service: validate\n    Service-->Gateway: accepted\n    Gateway-->Client: response",
    },
];
