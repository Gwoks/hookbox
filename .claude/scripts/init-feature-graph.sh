#!/usr/bin/env bash
# Bootstrap the full agent-workflow graph for a feature in beads.
#
#   init-feature-graph.sh <slug> "<feature title>"
#
# Creates the feature epic + the Stage-A "discovery" sub-graph with dependencies
# encoding the flow. The Stage-B "build" sub-graph (tasks + QA gate + sync) is
# created later by the product-manager in BREAKDOWN mode.
#
# Each agent claims its issue by label, e.g.:
#   bd ready -l step:journey,feature:<slug> --claim --json
set -euo pipefail

slug="${1:?usage: init-feature-graph.sh <slug> \"<title>\"}"
title="${2:-$slug}"
F="feature:$slug"

# create a child issue under the epic: mk "<title>" "<extra,labels>"
mk() { bd create "$1" -t task --parent "$EPIC" -l "$F,$2" --silent; }

EPIC=$(bd create "$title" -t epic -l "$F" --spec-id "$slug" \
  -d "Agent workflow for $slug. Artifacts in docs/features/$slug/." --silent)

DRAFT=$(mk     "PRD draft — product-manager"               "step:prd-draft,phase:discovery,agent:product-manager")
JOURNEY=$(mk   "User journey analysis — user-journey"      "step:journey,phase:discovery,agent:user-journey")
UX=$(mk        "UI/UX design — ui-ux"                      "step:ux,phase:discovery,agent:ui-ux")
DESIGN=$(mk    "Visual design — design-agent"             "step:design,phase:discovery,agent:design-agent")
COPY=$(mk      "Voice & copy — copywriter-engineer"       "step:copywriter,phase:discovery,agent:copywriter-engineer")
ARCH=$(mk      "Architecture & §5 contract — system-architect" "step:architecture,phase:discovery,agent:system-architect")
SECURITY=$(mk  "Security threat model — security-engineer" "step:security,phase:discovery,agent:security-engineer")
REVISE=$(mk    "PRD revise — product-manager"             "step:prd-revise,phase:discovery,agent:product-manager")
APPROVE=$(mk   "PRD approval — human gate"                "step:approval,phase:discovery,gate:human")
BREAKDOWN=$(mk "Task breakdown — product-manager"         "step:breakdown,phase:discovery,agent:product-manager")

# dependencies: bd dep add <blocked> <blocker>
# journey ∥ ux ∥ architecture ∥ security all unblock once the draft closes (run in parallel)
for b in "$JOURNEY" "$UX" "$ARCH" "$SECURITY"; do bd dep add "$b" "$DRAFT"  >/dev/null; done
# visual design is a handoff from ui-ux: it builds on ux.md, so it waits for ux to close
bd dep add "$DESIGN" "$UX" >/dev/null
# copy refines after design: it builds on ux.md + design.md, so it waits for design to close
bd dep add "$COPY" "$DESIGN" >/dev/null
# revise folds in all six critique/design/copy artifacts
for b in "$JOURNEY" "$UX" "$DESIGN" "$COPY" "$ARCH" "$SECURITY"; do bd dep add "$REVISE" "$b" >/dev/null; done
bd dep add "$APPROVE"   "$REVISE"   >/dev/null
bd dep add "$BREAKDOWN" "$APPROVE"  >/dev/null

bd dep cycles >/dev/null
echo "epic=$EPIC"
echo "Discovery graph ready under $EPIC. Inspect with: bd dep tree $EPIC"
