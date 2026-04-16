local function get_field(inv, key, default)
  if inv == nil then
    return default
  end

  if inv.attributes ~= nil and inv.attributes[key] ~= nil then
    return inv.attributes[key]
  end

  if type(inv.body) == "table" and inv.body[key] ~= nil then
    return inv.body[key]
  end

  return default
end

local function to_number(value, default)
  if type(value) == "number" then
    return value
  end

  if type(value) == "string" then
    local parsed = tonumber(value)
    if parsed ~= nil then
      return parsed
    end
  end

  return default
end

local function html_escape(value)
  local text = tostring(value or "")
  text = text:gsub("&", "&amp;")
  text = text:gsub("<", "&lt;")
  text = text:gsub(">", "&gt;")
  text = text:gsub('"', "&quot;")
  return text
end

mdx.register_directive("statblock", function(inv)
  local name = get_field(inv, "name")
  if name == nil or tostring(name) == "" then
    return {
      type = "error",
      message = "statblock: 'name' is required"
    }
  end

  local role = get_field(inv, "role", "")
  local faction = get_field(inv, "faction", "")
  local strength = to_number(get_field(inv, "strength"), 0)
  local agility = to_number(get_field(inv, "agility"), 0)
  local willpower = to_number(get_field(inv, "willpower"), 0)

  local html = string.format(
    '<div class="statblock"><h2>%s</h2><p><strong>Role:</strong> %s</p><p><strong>Faction:</strong> %s</p><ul><li><strong>STR:</strong> %d</li><li><strong>AGI:</strong> %d</li><li><strong>WIL:</strong> %d</li></ul></div>',
    html_escape(name),
    html_escape(role),
    html_escape(faction),
    strength,
    agility,
    willpower
  )

  return {
    type = "html",
    value = html
  }
end)
