local function html_escape(value)
  local text = tostring(value or "")
  text = text:gsub("&", "&amp;")
  text = text:gsub("<", "&lt;")
  text = text:gsub(">", "&gt;")
  text = text:gsub('"', "&quot;")
  return text
end

local function get_npc(ctx, target)
  if ctx == nil then
    return nil
  end

  local frontmatter = ctx.frontmatter or ctx.document_metadata
  if type(frontmatter) ~= "table" then
    return nil
  end

  local npcs = frontmatter.npcs
  if type(npcs) ~= "table" then
    return nil
  end

  local npc = npcs[target]
  if type(npc) ~= "table" then
    return nil
  end

  return npc
end

mdx.register_link_resolver("npc", function(link, ctx)
  local npc = get_npc(ctx, link.target)
  if npc == nil then
    return {
      type = "error",
      message = string.format("npc link: unknown target '%s'", link.target)
    }
  end

  local name = html_escape(npc.name or link.text or link.target)
  local href = html_escape(npc.href or ("#npc:" .. link.target))
  local role = html_escape(npc.role or "")
  local faction = html_escape(npc.faction or "")

  local details = role
  if role ~= "" and faction ~= "" then
    details = details .. " · " .. faction
  elseif faction ~= "" then
    details = faction
  end

  local chip = string.format(
    '<span class="npc-chip"><a class="npc-link" href="%s" data-role="%s" data-faction="%s">%s</a>',
    href,
    role,
    faction,
    name
  )

  if details ~= "" then
    chip = chip .. string.format("<small>%s</small>", details)
  end

  chip = chip .. "</span>"

  return {
    type = "html",
    value = chip
  }
end)
