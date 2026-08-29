// Phase C, milestone C5: thin client over the backend Action Orchestrator.
// This file used to independently re-fetch deadlines/tasks/waiting_for per
// matter and rank them with its own ad-hoc sort - a second prioritization
// system that could (and did) disagree with the backend's case_health.rs.
// It is now a pure pass-through: ranking, urgency, and reasoning all come
// from action_engine.rs; nothing here re-sorts or re-classifies anything.
import { commands } from "./ipc";
import type { ActionCandidate, ActionCenterEntry, ActionPlan } from "../types";

export async function loadActionCenter(): Promise<ActionCenterEntry[]> {
  return (await commands.get_action_center()) as ActionCenterEntry[];
}

export async function loadMatterActionPlan(matterId: string): Promise<ActionPlan> {
  return (await commands.get_matter_action_plan({ matterId })) as ActionPlan;
}

export async function setRecommendationState(
  matterId: string, fingerprint: string, state: string, snoozedUntil?: string, note?: string,
) {
  return commands.set_action_recommendation_state({ matterId, fingerprint, state, snoozedUntil, note });
}

export async function convertActionToTask(matterId: string, fingerprint: string, title?: string) {
  return commands.convert_action_to_task({ matterId, fingerprint, title });
}

export async function markDeadlineSatisfied(deadlineId: string, note?: string) {
  return commands.mark_deadline_satisfied({ deadlineId, note });
}

/// Flattens the global Action Center into rows for a simple list view,
/// preserving the backend's own matter ordering and each matter's own
/// candidate rank order - no re-sorting here.
export function flattenActionCenter(entries: ActionCenterEntry[]): ActionCandidate[] {
  return entries.flatMap(e => e.plan.candidates);
}
