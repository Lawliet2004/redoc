export function getTemplateBody(templateId: string): any {
  if (templateId === "resume") {
    return {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1, align: "center" },
          content: [{ type: "text", text: "Jane Doe" }],
        },
        {
          type: "paragraph",
          attrs: { align: "center" },
          content: [{ type: "text", text: "Software Engineer • jane.doe@example.com • (555) 019-2834 • City, State" }],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Professional Summary" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Experienced software developer with expertise in high-performance web applications and desktop tools. Proven track record of delivering clean, testable code." }],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Experience" }],
        },
        {
          type: "paragraph",
          content: [
            { type: "text", text: "Senior Engineer — Acme Corp", marks: [{ type: "bold" }] },
            { type: "text", text: " (2022 - Present)" },
          ],
        },
        {
          type: "bullet_list",
          content: [
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "Led development of core features and improved software build performance by 40%." }] }],
            },
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "Mentored junior engineers and instituted code review standards." }] }],
            },
          ],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "Education" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "B.S. in Computer Science — Tech University, 2021" }],
        },
      ],
    };
  }
  if (templateId === "report") {
    return {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Project Status & Progress Report" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Date: August 2026 | Prepared by: Development Team" }],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "1. Executive Summary" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "This report details the recent achievements, current status, and key roadmap milestones for the Redoc suite initiative." }],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "2. Accomplishments & Key Milestones" }],
        },
        {
          type: "bullet_list",
          content: [
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "Completed core editor architecture and shell integration." }] }],
            },
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "Implemented comprehensive test coverage across document, sheet, and slide modules." }] }],
            },
          ],
        },
        {
          type: "heading",
          attrs: { level: 2 },
          content: [{ type: "text", text: "3. Next Steps" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Finalize remaining file format import/export filters and conduct final performance verification." }],
        },
      ],
    };
  }
  if (templateId === "budget") {
    return {
      sheets: [
        {
          id: "sheet-1",
          name: "Budget Summary",
          cells: {
            "1:1": { raw: "Category", display: "Category", style: { bold: true } },
            "1:2": { raw: "Budgeted", display: "Budgeted", style: { bold: true } },
            "1:3": { raw: "Actual", display: "Actual", style: { bold: true } },
            "1:4": { raw: "Difference", display: "Difference", style: { bold: true } },
            "2:1": { raw: "Housing / Rent" },
            "2:2": { raw: "1500", display: "$1,500" },
            "2:3": { raw: "1500", display: "$1,500" },
            "2:4": { raw: "=C2-B2", display: "$0" },
            "3:1": { raw: "Utilities & Internet" },
            "3:2": { raw: "250", display: "$250" },
            "3:3": { raw: "230", display: "$230" },
            "3:4": { raw: "=C3-B3", display: "-$20" },
            "4:1": { raw: "Groceries & Food" },
            "4:2": { raw: "600", display: "$600" },
            "4:3": { raw: "650", display: "$650" },
            "4:4": { raw: "=C4-B4", display: "$50" },
            "5:1": { raw: "Transportation" },
            "5:2": { raw: "200", display: "$200" },
            "5:3": { raw: "180", display: "$180" },
            "5:4": { raw: "=C5-B5", display: "-$20" },
            "6:1": { raw: "Total", style: { bold: true } },
            "6:2": { raw: "=SUM(B2:B5)", display: "$2,550", style: { bold: true } },
            "6:3": { raw: "=SUM(C2:C5)", display: "$2,560", style: { bold: true } },
            "6:4": { raw: "=SUM(D2:D5)", display: "$10", style: { bold: true } },
          },
          colWidths: { "1": 160, "2": 100, "3": 100, "4": 100 },
          rowHeights: {},
          freezeRows: 1,
          freezeCols: 0,
        },
      ],
      activeSheetIndex: 0,
    };
  }
  if (templateId === "project_plan") {
    return {
      sheets: [
        {
          id: "sheet-1",
          name: "Project Roadmap",
          cells: {
            "1:1": { raw: "Task Name", display: "Task Name", style: { bold: true } },
            "1:2": { raw: "Owner", display: "Owner", style: { bold: true } },
            "1:3": { raw: "Start Date", display: "Start Date", style: { bold: true } },
            "1:4": { raw: "End Date", display: "End Date", style: { bold: true } },
            "1:5": { raw: "Status", display: "Status", style: { bold: true } },
            "2:1": { raw: "Requirement Gathering" },
            "2:2": { raw: "Alice" },
            "2:3": { raw: "2026-08-01" },
            "2:4": { raw: "2026-08-05" },
            "2:5": { raw: "Completed" },
            "3:1": { raw: "Architecture & Specs" },
            "3:2": { raw: "Bob" },
            "3:3": { raw: "2026-08-06" },
            "3:4": { raw: "2026-08-12" },
            "3:5": { raw: "In Progress" },
            "4:1": { raw: "Core Implementation" },
            "4:2": { raw: "Charlie" },
            "4:3": { raw: "2026-08-13" },
            "4:4": { raw: "2026-08-25" },
            "4:5": { raw: "Not Started" },
            "5:1": { raw: "Testing & QA" },
            "5:2": { raw: "Dana" },
            "5:3": { raw: "2026-08-26" },
            "5:4": { raw: "2026-08-31" },
            "5:5": { raw: "Not Started" },
          },
          colWidths: { "1": 180, "2": 100, "3": 110, "4": 110, "5": 120 },
          rowHeights: {},
          freezeRows: 1,
          freezeCols: 0,
        },
      ],
      activeSheetIndex: 0,
    };
  }
  if (templateId === "pitch_deck") {
    return {
      slides: [
        {
          id: "slide-1",
          title: "Title Slide",
          layout: "title",
          transition: "none",
          notes: "Introduce company vision and team.",
          elements: [
            {
              id: "el-1",
              type: "text",
              x: 80,
              y: 140,
              width: 800,
              height: 90,
              content: "Redoc Office Suite",
              fontSize: 42,
              align: "center",
              bold: true,
            },
            {
              id: "el-2",
              type: "text",
              x: 80,
              y: 250,
              width: 800,
              height: 50,
              content: "Next-Generation Productivity Platform",
              fontSize: 22,
              align: "center",
            },
          ],
        },
        {
          id: "slide-2",
          title: "Problem Statement",
          layout: "title-body",
          transition: "fade",
          notes: "Highlight key market pain points.",
          elements: [
            {
              id: "el-3",
              type: "text",
              x: 80,
              y: 50,
              width: 800,
              height: 60,
              content: "The Problem",
              fontSize: 32,
              align: "left",
              bold: true,
            },
            {
              id: "el-4",
              type: "text",
              x: 80,
              y: 140,
              width: 800,
              height: 240,
              content: "• Legacy office software is slow and monolithic.\n• Cloud-only tools lack robust offline capabilities.\n• Fragmented formats cause compatibility issues.",
              fontSize: 20,
              align: "left",
            },
          ],
        },
        {
          id: "slide-3",
          title: "The Solution",
          layout: "title-body",
          transition: "fade",
          notes: "Present Redoc's unified local-first architecture.",
          elements: [
            {
              id: "el-5",
              type: "text",
              x: 80,
              y: 50,
              width: 800,
              height: 60,
              content: "Our Solution",
              fontSize: 32,
              align: "left",
              bold: true,
            },
            {
              id: "el-6",
              type: "text",
              x: 80,
              y: 140,
              width: 800,
              height: 240,
              content: "• Unified desktop engine for Docs, Sheets, and Slides.\n• Blazing-fast native performance with zero telemetry lock-in.\n• Full interoperability with standard document formats.",
              fontSize: 20,
              align: "left",
            },
          ],
        },
      ],
      theme: {
        id: "default-light",
        name: "Modern Light",
        bgColor: "#ffffff",
        textColor: "#1e293b",
        accentColor: "#3b82f6",
        fontFamily: "Inter, sans-serif",
      },
    };
  }
  return null;
}
