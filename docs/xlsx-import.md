# XLSX chart import coverage

XLSX import now recovers basic chart objects from worksheet drawing anchors. It follows worksheet and drawing relationship parts, maps bar/line/pie chart types, captures a chart title when present, and restores the chart placement range into the sheet model.

Malformed or unmappable chart parts remain non-fatal: the importer emits a warning and preserves the rest of the workbook. Advanced chart features and pivot-chart semantics remain on the interoperability roadmap.
