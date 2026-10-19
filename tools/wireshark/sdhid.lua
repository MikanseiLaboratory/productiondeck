-- Post-dissector for Stream Deck HID reports inside USBPcap captures.
--
-- It reads the `usbhid.data` field produced by Wireshark's USB HID dissector
-- and annotates Stream Deck Gen1/Gen2 reports:
--   report 0x01 input   : cmd 0x00 buttons, 0x03 encoder, 0x04 NFC
--   report 0x02 output  : cmd 0x07 key image, 0x0c panel image, 0x0f/0x10 LEDs
--   report 0x03 feature : report id at byte 1
--
-- Usage: -X lua_script:tools/wireshark/sdhid.lua

local sdhid = Proto("sdhid", "Stream Deck HID")

local f = {}
f.report_id = ProtoField.uint8("sdhid.report_id", "Report ID", base.HEX)
f.command = ProtoField.uint8("sdhid.command", "Command", base.HEX)
f.key = ProtoField.uint8("sdhid.key", "Key index")
f.last = ProtoField.bool("sdhid.image.last", "Transfer done")
f.size = ProtoField.uint16("sdhid.image.size", "Chunk size")
f.index = ProtoField.uint16("sdhid.image.index", "Chunk index")
f.encoder = ProtoField.uint8("sdhid.encoder", "Encoder index")
f.feature_id = ProtoField.uint8("sdhid.feature.id", "Feature report id", base.HEX)

sdhid.fields = {
  f.report_id, f.command, f.key, f.last, f.size, f.index, f.encoder, f.feature_id,
}

local usbhid_data = Field.new("usbhid.data")

local function annotate(bytes, tree, pinfo)
  if #bytes < 2 then return end
  local rid = bytes:byte(1)
  local cmd = bytes:byte(2)
  tree:add(f.report_id, bytes:range(0, 1))
  if rid == 0x01 then
    if cmd == 0x00 then
      pinfo.cols.info:append(" SD buttons")
    elseif cmd == 0x03 then
      pinfo.cols.info:append(" SD encoder")
    elseif cmd == 0x04 then
      pinfo.cols.info:append(" SD NFC")
    end
  elseif rid == 0x02 then
    tree:add(f.command, bytes:range(1, 1))
    if cmd == 0x07 and #bytes >= 8 then
      tree:add(f.key, bytes:range(2, 1))
      tree:add(f.last, bytes:range(3, 1))
      tree:add_le(f.size, bytes:range(4, 2))
      tree:add_le(f.index, bytes:range(6, 2))
      pinfo.cols.info:append(string.format(" SD key image key=%d", bytes:byte(3)))
    elseif cmd == 0x0f or cmd == 0x10 then
      tree:add(f.encoder, bytes:range(2, 1))
      pinfo.cols.info:append(" SD encoder LED")
    elseif cmd == 0x0c then
      pinfo.cols.info:append(" SD panel image")
    end
  elseif rid == 0x03 then
    tree:add(f.feature_id, bytes:range(1, 1))
    pinfo.cols.info:append(string.format(" SD feature 0x%02x", cmd))
  end
end

function sdhid.postdissector(tvb, pinfo, tree)
  local values = { usbhid_data() }
  for _, value in ipairs(values) do
    if value then
      local bytes = value.range:bytes()
      annotate(bytes, tree, pinfo)
    end
  end
end

register_postdissector(sdhid)
