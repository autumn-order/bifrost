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

// Lua script to atomically pop the earliest job from the queue
// Uses ZPOPMIN to retrieve and remove the job with the lowest score (earliest timestamp)
// Only pops jobs that are due (score <= current time)
//
// KEYS[1]: sorted set key (queue name)
// ARGV[1]: current timestamp in milliseconds
//
// Returns:
//   nil if queue is empty or no jobs are due
//   table with {identity, score} if job was popped
pub static POP_JOB_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local now = tonumber(ARGV[1])

-- Get the earliest job without removing it
local peek = redis.call('ZRANGE', queue_key, 0, 0, 'WITHSCORES')

-- Check if queue is empty
if #peek == 0 then
    return nil
end

-- Check if the earliest job is due (score <= now)
local score = tonumber(peek[2])
if score > now then
    return nil
end

-- Job is due, pop it atomically
local result = redis.call('ZPOPMIN', queue_key, 1)

-- Return both identity and score as table
-- result[1] is the identity string, result[2] is the score
return {result[1], result[2]}
"#;

// Lua script to atomically pop multiple jobs from the queue
// Pops up to max_count jobs that are due for execution (score <= current time)
// More efficient than calling POP_JOB_SCRIPT in a loop as it requires only one Redis round-trip
//
// KEYS[1]: sorted set key (queue name)
// ARGV[1]: current timestamp in milliseconds
// ARGV[2]: maximum number of jobs to pop
//
// Returns:
//   empty table {} if queue is empty or no jobs are due
//   table with {identity1, score1, identity2, score2, ...} for popped jobs
pub static POP_BATCH_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local now = tonumber(ARGV[1])
local max_count = tonumber(ARGV[2])

local results = {}

-- Get up to max_count jobs with their scores
-- Jobs are sorted by score (timestamp), so we can stop at first non-due job
local jobs = redis.call('ZRANGE', queue_key, 0, max_count - 1, 'WITHSCORES')

-- Iterate through jobs in pairs (identity, score)
for i = 1, #jobs, 2 do
    local identity = jobs[i]
    local score = tonumber(jobs[i + 1])

    -- Only pop jobs that are due
    if score <= now then
        redis.call('ZREM', queue_key, identity)
        table.insert(results, identity)
        table.insert(results, score)
    else
        -- Jobs are sorted, remaining jobs also not due
        break
    end
end

return results
"#;

// Lua script to atomically push multiple jobs to the queue
// Checks for duplicates and adds all non-duplicate jobs in a single operation
//
// KEYS[1]: sorted set key (queue name)
// ARGV: pairs of (identity, score) - [identity1, score1, identity2, score2, ...]
//
// Returns:
//   number of jobs that were added (duplicates are skipped)
pub static PUSH_BATCH_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local added_count = 0

-- Process jobs in pairs (identity, score)
for i = 1, #ARGV, 2 do
    local identity = ARGV[i]
    local score = tonumber(ARGV[i + 1])

    -- Check if identity already exists in queue (O(1) operation)
    local exists = redis.call('ZSCORE', queue_key, identity)
    if not exists then
        -- No duplicate found, add identity to sorted set
        redis.call('ZADD', queue_key, score, identity)
        added_count = added_count + 1
    end
end

return added_count
"#;
