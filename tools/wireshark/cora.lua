-- Wireshark dissector for Elgato's Cora protocol (Stream Deck Studio / Network Dock).
--
-- Usage:
--   wireshark -X lua_script:tools/wireshark/cora.lua
--   tshark -X lua_script:tools/wireshark/cora.lua -r capture.pcapng -Y cora
--
-- Cora frame:
--   0..3   magic 43 93 8A 41
--   4..5   flags (u16 LE)
--   6      hidOp
--   7      reserved
--   8..11  messageId (u32 LE)
--   12..15 payloadLen (u32 LE)
--   16..   payload

local cora = Proto("cora", "Stream Deck Cora")

local f = {}
f.magic = ProtoField.bytes("sdcora.magic", "Magic")
f.flags = ProtoField.uint16("sdcora.flags", "Flags", base.HEX)
f.flags_verbatim = ProtoField.bool("sdcora.flags.verbatim", "VERBATIM", 16, nil, 0x8000)
f.flags_reqack = ProtoField.bool("sdcora.flags.req_ack", "REQ_ACK", 16, nil, 0x4000)
f.flags_acknak = ProtoField.bool("sdcora.flags.ack_nak", "ACK_NAK", 16, nil, 0x0200)
f.flags_result = ProtoField.bool("sdcora.flags.result", "RESULT", 16, nil, 0x0100)
f.hid_op = ProtoField.uint8("sdcora.hid_op", "HID operation", base.HEX, {
  [0] = "WRITE (hid_write)",
  [1] = "SEND_REPORT (hid_send_feature_report)",
  [2] = "GET_REPORT (hid_get_feature_report)",
})
f.reserved = ProtoField.uint8("sdcora.reserved", "Reserved", base.HEX)
f.message_id = ProtoField.uint32("sdcora.message_id", "Message ID", base.HEX)
f.payload_len = ProtoField.uint32("sdcora.payload_len", "Payload length")
f.payload = ProtoField.bytes("sdcora.payload", "Payload")
f.payload_type = ProtoField.uint8("sdcora.payload_type", "Payload type", base.HEX, {
  [0x01] = "Event / input",
  [0x02] = "Output report",
  [0x03] = "Feature report",
})
f.event_subtype = ProtoField.uint8("sdcora.event_subtype", "Event subtype", base.HEX, {
  [0x00] = "Buttons",
  [0x02] = "Touch",
  [0x03] = "Encoder",
  [0x04] = "NFC",
  [0x0a] = "Keep-alive",
  [0x0b] = "Capabilities",
})
f.keepalive_conn = ProtoField.uint8("sdcora.keepalive.conn", "Connection number")
f.caps_connected = ProtoField.uint8("sdcora.caps.connected", "Connected")
f.caps_rows = ProtoField.uint8("sdcora.caps.rows_count", "Rows")
f.caps_cols = ProtoField.uint8("sdcora.caps.columns", "Columns")
f.caps_key_w = ProtoField.uint16("sdcora.caps.key_w", "Key width")
f.caps_key_h = ProtoField.uint16("sdcora.caps.key_h", "Key height")
f.caps_vid = ProtoField.uint16("sdcora.caps.vid", "VID", base.HEX)
f.caps_pid = ProtoField.uint16("sdcora.caps.pid", "PID", base.HEX)
f.caps_model = ProtoField.string("sdcora.caps.model", "Model name")
f.caps_serial = ProtoField.string("sdcora.caps.serial", "Serial")
f.caps_child_port = ProtoField.uint16("sdcora.caps.child_port", "Child TCP port")
f.feature_id = ProtoField.uint8("sdcora.feature.id", "Report id", base.HEX)
f.serial_len = ProtoField.uint8("sdcora.serial.len", "Serial length")
f.serial = ProtoField.string("sdcora.serial.value", "Serial")
f.fw_version = ProtoField.string("sdcora.fw.version", "Firmware version")
f.mac = ProtoField.bytes("sdcora.mac", "MAC address")
f.child_status = ProtoField.uint8("sdcora.child.status", "Child status", base.HEX)
f.child_vid = ProtoField.uint16("sdcora.child.vid", "Child VID", base.HEX)
f.child_pid = ProtoField.uint16("sdcora.child.pid", "Child PID", base.HEX)
f.child_serial = ProtoField.string("sdcora.child.serial", "Child serial")
f.child_port = ProtoField.uint16("sdcora.child.port", "Child port")
f.image_cmd = ProtoField.uint8("sdcora.image.cmd", "Image command", base.HEX)
f.image_key = ProtoField.uint8("sdcora.image.key", "Key index")
f.image_last = ProtoField.bool("sdcora.image.last", "Transfer done")
f.image_size = ProtoField.uint16("sdcora.image.size", "Chunk size")
f.image_index = ProtoField.uint16("sdcora.image.index", "Chunk index")
f.image_x = ProtoField.uint16("sdcora.image.x", "X")
f.image_y = ProtoField.uint16("sdcora.image.y", "Y")
f.image_w = ProtoField.uint16("sdcora.image.w", "Width")
f.image_h = ProtoField.uint16("sdcora.image.h", "Height")
f.encoder = ProtoField.uint8("sdcora.encoder", "Encoder index")
f.encoder_led = ProtoField.bytes("sdcora.encoder.led", "Encoder LED data")

cora.fields = {
  f.magic, f.flags, f.flags_verbatim, f.flags_reqack, f.flags_acknak, f.flags_result,
  f.hid_op, f.reserved, f.message_id, f.payload_len, f.payload, f.payload_type,
  f.event_subtype, f.keepalive_conn, f.caps_connected, f.caps_rows, f.caps_cols,
  f.caps_key_w, f.caps_key_h, f.caps_vid, f.caps_pid, f.caps_model, f.caps_serial,
  f.caps_child_port, f.feature_id, f.serial_len, f.serial, f.fw_version, f.mac,
  f.child_status, f.child_vid, f.child_pid, f.child_serial, f.child_port,
  f.image_cmd, f.image_key, f.image_last, f.image_size, f.image_index, f.image_x,
  f.image_y, f.image_w, f.image_h, f.encoder, f.encoder_led,
}

local function has_bit(value, mask)
  return math.floor(value / mask) % 2 == 1
end

local function add_flags(tree, buffer, offset)
  local flags = buffer(offset, 2):le_uint()
  local fl = tree:add_le(f.flags, buffer(offset, 2))
  if has_bit(flags, 0x8000) then fl:add(f.flags_verbatim, buffer(offset, 2)) end
  if has_bit(flags, 0x4000) then fl:add(f.flags_reqack, buffer(offset, 2)) end
  if has_bit(flags, 0x0200) then fl:add(f.flags_acknak, buffer(offset, 2)) end
  if has_bit(flags, 0x0100) then fl:add(f.flags_result, buffer(offset, 2)) end
  return flags
end

local function add_ascii(tree, field, tvb, offset, len)
  if offset + len <= tvb:len() then
    local s = tvb(offset, len):string()
    s = s:gsub("%z+$", "")
    tree:add(field, tvb(offset, len), s)
  end
end

local function dissect_event(tree, tvb, pinfo)
  if tvb:len() < 2 then return end
  local sub = tvb(1, 1):uint()
  tree:add(f.event_subtype, tvb(1, 1))
  if sub == 0x0a then
    if tvb:len() > 5 then
      tree:add(f.keepalive_conn, tvb(5, 1))
    end
    pinfo.cols.info:append(" keep-alive")
  elseif sub == 0x0b and tvb:len() >= 128 then
    tree:add(f.caps_connected, tvb(4, 1))
    tree:add(f.caps_rows, tvb(5, 1))
    tree:add(f.caps_cols, tvb(6, 1))
    tree:add_le(f.caps_key_w, tvb(8, 2))
    tree:add_le(f.caps_key_h, tvb(10, 2))
    tree:add_le(f.caps_vid, tvb(26, 2))
    tree:add_le(f.caps_pid, tvb(28, 2))
    add_ascii(tree, f.caps_model, tvb, 62, 32)
    add_ascii(tree, f.caps_serial, tvb, 94, 31)
    tree:add_le(f.caps_child_port, tvb(126, 2))
    pinfo.cols.info:append(" capabilities")
  elseif sub == 0x00 then
    pinfo.cols.info:append(" buttons")
  elseif sub == 0x03 then
    if tvb:len() > 4 then
      local subtype = tvb(4, 1):uint()
      pinfo.cols.info:append(subtype == 1 and " encoder rotate" or " encoder press")
    end
  elseif sub == 0x04 then
    if tvb:len() >= 3 then
      local n = tvb(1, 1):uint() + tvb(2, 1):uint() * 256
      pinfo.cols.info:append(string.format(" NFC len=%d", n))
    end
  elseif sub == 0x02 then
    pinfo.cols.info:append(" touch")
  end
end

local function dissect_feature(tree, tvb, pinfo)
  if tvb:len() < 2 then return end
  local id = tvb(1, 1):uint()
  tree:add(f.feature_id, tvb(1, 1))
  pinfo.cols.info:append(string.format(" feature 0x%02x", id))
  if id == 0x80 and tvb:len() >= 16 then
    tree:add_le(f.caps_vid, tvb(12, 2))
    tree:add_le(f.caps_pid, tvb(14, 2))
  elseif (id == 0x83 or id == 0x86 or id == 0x8a or id == 0x8f) and tvb:len() >= 16 then
    add_ascii(tree, f.fw_version, tvb, 8, 8)
  elseif id == 0x84 and tvb:len() >= 4 then
    local n = tvb(3, 1):uint()
    tree:add(f.serial_len, tvb(3, 1))
    add_ascii(tree, f.serial, tvb, 4, n)
  elseif id == 0x85 and tvb:len() >= 10 then
    tree:add(f.mac, tvb(4, 6))
  elseif id == 0x1c and tvb:len() >= 128 then
    tree:add(f.child_status, tvb(4, 1))
    tree:add_le(f.child_vid, tvb(26, 2))
    tree:add_le(f.child_pid, tvb(28, 2))
    add_ascii(tree, f.child_serial, tvb, 94, 31)
    tree:add_le(f.child_port, tvb(126, 2))
  end
end

local function dissect_output(tree, tvb, pinfo)
  if tvb:len() < 2 then return end
  local cmd = tvb(1, 1):uint()
  tree:add(f.image_cmd, tvb(1, 1))
  if cmd == 0x07 and tvb:len() >= 8 then
    tree:add(f.image_key, tvb(2, 1))
    tree:add(f.image_last, tvb(3, 1))
    tree:add_le(f.image_size, tvb(4, 2))
    tree:add_le(f.image_index, tvb(6, 2))
    pinfo.cols.info:append(string.format(" key image key=%d", tvb(2, 1):uint()))
  elseif cmd == 0x0c and tvb:len() >= 16 then
    tree:add_le(f.image_x, tvb(2, 2))
    tree:add_le(f.image_y, tvb(4, 2))
    tree:add_le(f.image_w, tvb(6, 2))
    tree:add_le(f.image_h, tvb(8, 2))
    tree:add(f.image_last, tvb(10, 1))
    tree:add_le(f.image_index, tvb(11, 2))
    tree:add_le(f.image_size, tvb(13, 2))
    pinfo.cols.info:append(" panel image")
  elseif cmd == 0x0f then
    tree:add(f.encoder, tvb(2, 1))
    tree:add(f.encoder_led, tvb(3, tvb:len() - 3))
    pinfo.cols.info:append(" encoder ring")
  elseif cmd == 0x10 then
    tree:add(f.encoder, tvb(2, 1))
    tree:add(f.encoder_led, tvb(3, 3))
    pinfo.cols.info:append(" encoder LED")
  elseif cmd == 0x08 then
    pinfo.cols.info:append(" fullscreen image")
  end
end

local function dissect_payload(tree, tvb, pinfo)
  if tvb:len() < 1 then return end
  local typ = tvb(0, 1):uint()
  tree:add(f.payload_type, tvb(0, 1))
  if typ == 0x01 then
    dissect_event(tree, tvb, pinfo)
  elseif typ == 0x03 then
    dissect_feature(tree, tvb, pinfo)
  elseif typ == 0x02 then
    dissect_output(tree, tvb, pinfo)
  end
end

local function dissect_cora_pdu(tvb, pinfo, tree)
  local root = tree:add(cora, tvb(), "Stream Deck Cora")
  if tvb:len() < 16 then return end
  root:add(f.magic, tvb(0, 4))
  local magic = tvb(0, 4):bytes():tohex():lower()
  if magic ~= "43938a41" then
    pinfo.cols.info = "Invalid Cora magic"
    return
  end
  local flags = add_flags(root, tvb, 4)
  root:add(f.hid_op, tvb(6, 1))
  root:add(f.reserved, tvb(7, 1))
  root:add_le(f.message_id, tvb(8, 4))
  local plen = tvb(12, 4):le_uint()
  root:add_le(f.payload_len, tvb(12, 4))
  local data = tvb(16)
  root:add(f.payload, data)
  pinfo.cols.protocol = "CORA"
  pinfo.cols.info = string.format("flags=0x%04x msg=0x%x len=%d", flags, tvb(8, 4):le_uint(), plen)
  local subtree = root:add(cora, data, "Payload")
  dissect_payload(subtree, data, pinfo)
end

function cora.dissector(tvb, pinfo, tree)
  local offset = 0
  while offset + 16 <= tvb:len() do
    local plen = tvb(offset + 12, 4):le_uint()
    local total = 16 + plen
    if offset + total > tvb:len() then
      pinfo.desegment_offset = offset
      pinfo.desegment_len = offset + total - tvb:len()
      return
    end
    dissect_cora_pdu(tvb:range(offset, total), pinfo, tree)
    offset = offset + total
  end
  if offset < tvb:len() then
    pinfo.desegment_offset = offset
    pinfo.desegment_len = DESEGMENT_ONE_MORE_SEGMENT or 0xFFFFFFFF
  end
end

local function heuristic(tvb, pinfo, tree)
  if tvb:len() >= 4 and tvb(0, 4):bytes():tohex() == "43938a41" then
    cora.dissector(tvb, pinfo, tree)
    return true
  end
  return false
end

-- Register on the fixed Cora ports and as a heuristic for dynamic child ports.
local tcp_port = DissectorTable.get("tcp.port")
tcp_port:add(5343, cora)
tcp_port:add(5344, cora)
tcp_port:add(20001, cora) -- observed Network Dock child port
cora:register_heuristic("tcp", heuristic)







