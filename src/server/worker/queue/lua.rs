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

// Lua script to get all jobs of a specific type without removing them
// Uses pattern matching on identity strings to filter jobs by type
//
// KEYS[1]: sorted set key (queue name)
// ARGV[1]: identity prefix pattern (e.g., "character:info:", "alliance:info:", "affiliation:batch:")
//
// Returns:
//   empty table if no matching jobs found
//   table of {identity, score} pairs for all matching jobs
pub static GET_ALL_OF_TYPE_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local prefix = ARGV[1]

-- Get all jobs from the sorted set with their scores
local all_jobs = redis.call('ZRANGE', queue_key, 0, -1, 'WITHSCORES')

-- Filter jobs that match the prefix
local matching_jobs = {}
for i = 1, #all_jobs, 2 do
    local identity = all_jobs[i]
    local score = all_jobs[i + 1]

    -- Check if identity starts with the prefix
    if string.sub(identity, 1, #prefix) == prefix then
        table.insert(matching_jobs, identity)
        table.insert(matching_jobs, score)
    end
end

return matching_jobs
"#;
