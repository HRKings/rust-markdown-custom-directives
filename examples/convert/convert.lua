-- Expected by mdx_lua:
--   mdx.register_directive("name", function(node, ctx) ... end)

local function get_attr(node, key, default)
  if node == nil or node.attributes == nil then
    return default
  end

  local value = node.attributes[key]
  if value == nil then
    return default
  end

  return value
end

local function to_number(v)
  if type(v) == "number" then
    return v
  end

  if type(v) == "string" then
    return tonumber(v)
  end

  return nil
end

local function get_league_km(ctx)
  if ctx == nil then
    return nil
  end

  local frontmatter = ctx.frontmatter or ctx.document_metadata
  if type(frontmatter) ~= "table" then
    return nil
  end

  local units = frontmatter.units
  if type(units) ~= "table" then
    return nil
  end

  local league_km = units.league_km
  if type(league_km) == "number" then
    return league_km
  end

  if type(league_km) == "string" then
    return tonumber(league_km)
  end

  return nil
end

mdx.register_directive("convert", function(node, ctx)
  local value = to_number(get_attr(node, "value"))
  local from = tostring(get_attr(node, "from", ""))
  local to = tostring(get_attr(node, "to", ""))

  if value == nil then
    return {
      type = "error",
      message = "convert: 'value' must be numeric"
    }
  end

  if from == "league" and to == "km" then
    local factor = get_league_km(ctx) or 4.8
    local result = value * factor

    return {
      type = "text",
      value = string.format("%.2f km", result)
    }
  end

  return {
    type = "error",
    message = string.format("convert: unsupported conversion '%s' -> '%s'", from, to)
  }
end)
