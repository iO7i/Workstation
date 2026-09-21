-- ?1 project, ?2 effective-as-of, ?3 knowledge-as-of; no "latest timestamp wins".
SELECT d.payload
FROM decisions d JOIN decision_acceptances a ON a.decision_id=d.id
WHERE d.project_id=?1 AND d.effective_at<=?2 AND d.recorded_at<=?3 AND a.accepted_at<=?3
AND NOT EXISTS (
 SELECT 1 FROM decision_acceptances successor
 JOIN decisions child ON child.id=successor.decision_id
 WHERE successor.predecessor=d.id AND successor.accepted_at<=?3
 AND child.recorded_at<=?3 AND child.effective_at<=?2
)
ORDER BY d.topic,d.scope,d.id
LIMIT 101;
