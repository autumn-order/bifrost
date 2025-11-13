//! Lua scripts for WorkerJobQueue redis implementation

// Lua script to atomically check for duplicates and add job to sorted set
// Uses ZSET for scheduling with identity as member (leverages native uniqueness)
// Identity contains job type and minimal data for duplicate detection:
//   - Simple jobs (character/alliance/corporation): full ID in identity string
//   - Affiliation batches: count and hash only (IDs must be retrieved from database)
//
// KEYS[1]: sorted set key (queue name)
// ARGV[1]: identity string
// ARGV[2]: score (timestamp)
//
// Returns:
//   1 if job was added
//   0 if job with same identity already exists
pub static PUSH_JOB_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local identity = ARGV[1]
local score = tonumber(ARGV[2])

-- Check if identity already exists in queue (O(1) operation)
local exists = redis.call('ZSCORE', queue_key, identity)
if exists then
    return 0
end

-- No duplicate found, add identity to sorted set
-- Identity contains all job data, so no separate storage needed
redis.call('ZADD', queue_key, score, identity)
return 1
"#;

// Lua script to remove stale jobs from the queue
// Removes all jobs with score (timestamp) older than the provided cutoff
//
// KEYS[1]: sorted set key (queue name)
// ARGV[1]: cutoff score (timestamp) - jobs older than this will be removed
//
// Returns: number of jobs removed
pub static CLEANUP_STALE_JOBS_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local cutoff_score = tonumber(ARGV[1])

-- Remove all jobs with score less than cutoff (ZREMRANGEBYSCORE is O(log(N)+M))
local removed = redis.call('ZREMRANGEBYSCORE', queue_key, '-inf', cutoff_score)
return removed
"#;
