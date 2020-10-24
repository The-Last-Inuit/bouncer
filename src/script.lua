redis.replicate_commands()
if not ARGV[1] or not ARGV[2] then
  return redis.error_reply("You have to provide a rate limit and the wait time.")
end
local calls_timestamps_list_name = KEYS[1]
local limit, wait_time = tonumber(ARGV[1]), tonumber(ARGV[2])
local current_timestamp = redis.call('time')
local current_timestamp_sec, current_mic_sec = current_timestamp[1], current_timestamp[2]
local return_val = {}
local calls_timestamps = redis.call('lrange', calls_timestamps_list_name, 0, -1)
table.sort(calls_timestamps)
local number_of_calls = #calls_timestamps

if number_of_calls < limit then
  redis.call('lpush', calls_timestamps_list_name, current_timestamp_sec)
  return_val.first = true
  return cjson.encode(return_val)
else
  local oldest_call_timestamp = calls_timestamps[1]
  local time_since_last_call = current_timestamp_sec - oldest_call_timestamp
  if wait_time <= time_since_last_call then
    redis.call('rpop', calls_timestamps_list_name)
    redis.call('lpush', calls_timestamps_list_name, current_timestamp_sec)
    return cjson.encode(return_val)
  else
    wait_time = wait_time - time_since_last_call
    return_val.wait = { wait_time, current_mic_sec + 5 }
    return_val.since = time_since_last_call
    return_val.old = oldest_call_timestamp
    return_val.current = current_timestamp_sec
    return cjson.encode(return_val)
  end
end

-- if no other api call has been performed for 1 minute then expire
redis.call('expire', calls_timestamps_list_name, 60)

