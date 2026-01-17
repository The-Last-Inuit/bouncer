redis.replicate_commands()
if not ARGV[1] or not ARGV[2] then
  return redis.error_reply("You have to provide a rate limit and the wait time.")
end

local key = KEYS[1]
local limit = tonumber(ARGV[1])
local wait_time_sec = tonumber(ARGV[2])

local current_timestamp = redis.call('time')
local current_timestamp_sec = tonumber(current_timestamp[1])
local current_timestamp_usec = tonumber(current_timestamp[2])
local current_usec = current_timestamp_sec * 1000000 + current_timestamp_usec
local window_usec = wait_time_sec * 1000000

local return_val = {
  allowed = false,
  old = 0,
  current = 0,
  since = 0,
  wait = { secs = 0, nanos = 0 }
}

local window_start = current_usec - window_usec
redis.call('zremrangebyscore', key, '-inf', window_start)

local number_of_calls = redis.call('zcard', key)

local function record_call()
  redis.call('zadd', key, current_usec, tostring(current_usec))
end

if number_of_calls < limit then
  record_call()
  return_val.allowed = true
else
  local oldest = redis.call('zrange', key, 0, 0, 'withscores')
  if oldest[2] then
    local oldest_usec = tonumber(oldest[2])
    local elapsed_usec = current_usec - oldest_usec
    local remaining_usec = window_usec - elapsed_usec
    if remaining_usec <= 0 then
      record_call()
      return_val.allowed = true
    else
      local wait_secs = math.floor(remaining_usec / 1000000)
      local wait_nanos = (remaining_usec % 1000000) * 1000
      return_val.wait = { secs = wait_secs, nanos = wait_nanos }
      return_val.since = math.floor(elapsed_usec / 1000000)
      return_val.old = math.floor(oldest_usec / 1000000)
      return_val.current = current_timestamp_sec
    end
  else
    record_call()
    return_val.allowed = true
  end
end

-- if no other api call has been performed for wait_time then expire
if wait_time_sec > 0 then
  redis.call('expire', key, wait_time_sec)
end

return cjson.encode(return_val)
