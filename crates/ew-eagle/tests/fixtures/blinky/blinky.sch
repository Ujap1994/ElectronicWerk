<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE eagle SYSTEM "eagle.dtd">
<eagle version="9.6.2">
<drawing>
<settings>
<setting alwaysvectorfont="no"/>
<setting verticaltext="up"/>
</settings>
<grid distance="0.1" unitdist="inch" unit="inch" style="lines" multiple="1" display="no"/>
<layers>
<layer number="1" name="Top" color="7" fill="1" visible="yes" active="yes"/>
<layer number="2" name="Route2" color="7" fill="1" visible="yes" active="yes"/>
<layer number="15" name="Route15" color="7" fill="1" visible="yes" active="yes"/>
<layer number="16" name="Bottom" color="7" fill="1" visible="yes" active="yes"/>
<layer number="17" name="Pads" color="7" fill="1" visible="yes" active="yes"/>
<layer number="18" name="Vias" color="7" fill="1" visible="yes" active="yes"/>
<layer number="19" name="Unrouted" color="7" fill="1" visible="yes" active="yes"/>
<layer number="20" name="Dimension" color="7" fill="1" visible="yes" active="yes"/>
<layer number="21" name="tPlace" color="7" fill="1" visible="yes" active="yes"/>
<layer number="22" name="bPlace" color="7" fill="1" visible="yes" active="yes"/>
<layer number="25" name="tNames" color="7" fill="1" visible="yes" active="yes"/>
<layer number="26" name="bNames" color="7" fill="1" visible="yes" active="yes"/>
<layer number="27" name="tValues" color="7" fill="1" visible="yes" active="yes"/>
<layer number="28" name="bValues" color="7" fill="1" visible="yes" active="yes"/>
<layer number="29" name="tStop" color="7" fill="1" visible="yes" active="yes"/>
<layer number="30" name="bStop" color="7" fill="1" visible="yes" active="yes"/>
<layer number="31" name="tCream" color="7" fill="1" visible="yes" active="yes"/>
<layer number="32" name="bCream" color="7" fill="1" visible="yes" active="yes"/>
<layer number="35" name="tGlue" color="7" fill="1" visible="yes" active="yes"/>
<layer number="36" name="bGlue" color="7" fill="1" visible="yes" active="yes"/>
<layer number="39" name="tKeepout" color="7" fill="1" visible="yes" active="yes"/>
<layer number="40" name="bKeepout" color="7" fill="1" visible="yes" active="yes"/>
<layer number="41" name="tRestrict" color="7" fill="1" visible="yes" active="yes"/>
<layer number="42" name="bRestrict" color="7" fill="1" visible="yes" active="yes"/>
<layer number="43" name="vRestrict" color="7" fill="1" visible="yes" active="yes"/>
<layer number="44" name="Drills" color="7" fill="1" visible="yes" active="yes"/>
<layer number="45" name="Holes" color="7" fill="1" visible="yes" active="yes"/>
<layer number="46" name="Milling" color="7" fill="1" visible="yes" active="yes"/>
<layer number="47" name="Measures" color="7" fill="1" visible="yes" active="yes"/>
<layer number="48" name="Document" color="7" fill="1" visible="yes" active="yes"/>
<layer number="49" name="Reference" color="7" fill="1" visible="yes" active="yes"/>
<layer number="51" name="tDocu" color="7" fill="1" visible="yes" active="yes"/>
<layer number="52" name="bDocu" color="7" fill="1" visible="yes" active="yes"/>
<layer number="91" name="Nets" color="7" fill="1" visible="yes" active="yes"/>
<layer number="92" name="Busses" color="7" fill="1" visible="yes" active="yes"/>
<layer number="93" name="Pins" color="7" fill="1" visible="yes" active="yes"/>
<layer number="94" name="Symbols" color="7" fill="1" visible="yes" active="yes"/>
<layer number="95" name="Names" color="7" fill="1" visible="yes" active="yes"/>
<layer number="96" name="Values" color="7" fill="1" visible="yes" active="yes"/>
<layer number="97" name="Info" color="7" fill="1" visible="yes" active="yes"/>
<layer number="98" name="Guide" color="7" fill="1" visible="yes" active="yes"/>
</layers>
<schematic xreflabel="%F%N/%S.%C%R" xrefpart="/%S.%C%R">
<description>Synthetic 555 blinker (test data)</description>
<libraries>
<library name="blinky">
<description>Synthetic test library</description>
<packages>
<package name="R0805">
<description>Synthetic R0805</description>
<smd name="1" x="-0.95" y="0" dx="1.3" dy="1.5" layer="1"/>
<smd name="2" x="0.95" y="0" dx="1.3" dy="1.5" layer="1"/>
<wire x1="-0.41" y1="0.635" x2="0.41" y2="0.635" width="0.1524" layer="21"/>
<wire x1="-0.41" y1="-0.635" x2="0.41" y2="-0.635" width="0.1524" layer="21"/>
<rectangle x1="-1" y1="-0.65" x2="1" y2="0.65" layer="51"/>
<text x="-1.27" y="1.27" size="0.8" layer="25">&gt;NAME</text>
<text x="-1.27" y="-2" size="0.8" layer="27">&gt;VALUE</text>
</package>
<package name="C0603">
<description>Synthetic C0603</description>
<smd name="1" x="-0.85" y="0" dx="1.1" dy="1" layer="1" roundness="25"/>
<smd name="2" x="0.85" y="0" dx="1.1" dy="1" layer="1" roundness="25"/>
<wire x1="-0.356" y1="0.432" x2="0.356" y2="0.432" width="0.1016" layer="51"/>
<text x="-1.27" y="1.27" size="0.8" layer="25">&gt;NAME</text>
<text x="-1.27" y="-2" size="0.8" layer="27">&gt;VALUE</text>
</package>
<package name="SO08">
<description>Synthetic SO08</description>
<smd name="1" x="-1.905" y="-2.6" dx="0.6" dy="2.2" layer="1"/>
<smd name="2" x="-0.635" y="-2.6" dx="0.6" dy="2.2" layer="1"/>
<smd name="3" x="0.635" y="-2.6" dx="0.6" dy="2.2" layer="1"/>
<smd name="4" x="1.905" y="-2.6" dx="0.6" dy="2.2" layer="1"/>
<smd name="5" x="1.905" y="2.6" dx="0.6" dy="2.2" layer="1"/>
<smd name="6" x="0.635" y="2.6" dx="0.6" dy="2.2" layer="1"/>
<smd name="7" x="-0.635" y="2.6" dx="0.6" dy="2.2" layer="1"/>
<smd name="8" x="-1.905" y="2.6" dx="0.6" dy="2.2" layer="1"/>
<wire x1="2.4" y1="1.9" x2="2.4" y2="-1.9" width="0.2032" layer="21"/>
<wire x1="-2.4" y1="-1.9" x2="-2.4" y2="1.9" width="0.2032" layer="21"/>
<circle x="-1.9" y="-1.3" radius="0.3" width="0" layer="21"/>
<text x="-1.27" y="1.27" size="0.8" layer="25">&gt;NAME</text>
<text x="-1.27" y="-2" size="0.8" layer="27">&gt;VALUE</text>
</package>
<package name="LED3MM">
<description>Synthetic LED3MM</description>
<pad name="A" x="-1.27" y="0" drill="0.8" diameter="1.6" shape="square"/>
<pad name="C" x="1.27" y="0" drill="0.8" diameter="1.6"/>
<circle x="0" y="0" radius="1.5" width="0.2032" layer="21"/>
<text x="-1.27" y="1.27" size="0.8" layer="25">&gt;NAME</text>
<text x="-1.27" y="-2" size="0.8" layer="27">&gt;VALUE</text>
</package>
<package name="HDR2">
<description>Synthetic HDR2</description>
<pad name="1" x="0" y="-1.27" drill="0.8" diameter="1.2" shape="long" rot="R90"/>
<pad name="2" x="0" y="1.27" drill="1" diameter="1.7" shape="octagon"/>
<wire x1="-1.27" y1="-2.54" x2="1.27" y2="-2.54" width="0.1524" layer="21"/>
<text x="-1.27" y="1.27" size="0.8" layer="25">&gt;NAME</text>
<text x="-1.27" y="-2" size="0.8" layer="27">&gt;VALUE</text>
</package>
</packages>
<symbols>
<symbol name="R">
<wire x1="-2.54" y1="-0.889" x2="2.54" y2="-0.889" width="0.254" layer="94"/>
<wire x1="2.54" y1="0.889" x2="-2.54" y2="0.889" width="0.254" layer="94"/>
<wire x1="2.54" y1="-0.889" x2="2.54" y2="0.889" width="0.254" layer="94"/>
<wire x1="-2.54" y1="-0.889" x2="-2.54" y2="0.889" width="0.254" layer="94"/>
<text x="-3.81" y="1.4986" size="1.778" layer="95">&gt;NAME</text>
<text x="-3.81" y="-3.302" size="1.778" layer="96">&gt;VALUE</text>
<pin name="1" x="-5.08" y="0" visible="off" length="short" direction="pas"/>
<pin name="2" x="5.08" y="0" visible="off" length="short" direction="pas" rot="R180"/>
</symbol>
<symbol name="C">
<rectangle x1="-2.032" y1="-2.032" x2="2.032" y2="-1.524" layer="94"/>
<rectangle x1="-2.032" y1="-1.016" x2="2.032" y2="-0.508" layer="94"/>
<wire x1="0" y1="0" x2="0" y2="-0.508" width="0.1524" layer="94"/>
<wire x1="0" y1="-2.54" x2="0" y2="-2.032" width="0.1524" layer="94"/>
<text x="1.016" y="0.4826" size="1.778" layer="95">&gt;NAME</text>
<text x="1.016" y="-4.191" size="1.778" layer="96">&gt;VALUE</text>
<pin name="1" x="0" y="2.54" visible="off" length="short" direction="pas" rot="R270"/>
<pin name="2" x="0" y="-5.08" visible="off" length="short" direction="pas" rot="R90"/>
</symbol>
<symbol name="LED">
<wire x1="1.27" y1="0" x2="0" y2="-2.54" width="0.254" layer="94"/>
<wire x1="0" y1="-2.54" x2="-1.27" y2="0" width="0.254" layer="94"/>
<wire x1="1.27" y1="-2.54" x2="-1.27" y2="-2.54" width="0.254" layer="94"/>
<wire x1="-1.27" y1="0" x2="1.27" y2="0" width="0.254" layer="94"/>
<wire x1="2" y1="-1" x2="3" y2="0" width="0.1524" layer="94" curve="-90"/>
<circle x="0" y="-1.27" radius="2.2" width="0.1524" layer="94"/>
<text x="3.556" y="-4.572" size="1.778" layer="95">&gt;NAME</text>
<text x="5.715" y="-4.572" size="1.778" layer="96">&gt;VALUE</text>
<pin name="A" x="0" y="2.54" visible="off" length="short" direction="pas" rot="R270"/>
<pin name="C" x="0" y="-5.08" visible="off" length="short" direction="pas" rot="R90"/>
</symbol>
<symbol name="NE555">
<wire x1="-7.62" y1="-10.16" x2="7.62" y2="-10.16" width="0.4064" layer="94"/>
<wire x1="7.62" y1="-10.16" x2="7.62" y2="10.16" width="0.4064" layer="94"/>
<wire x1="7.62" y1="10.16" x2="-7.62" y2="10.16" width="0.4064" layer="94"/>
<wire x1="-7.62" y1="10.16" x2="-7.62" y2="-10.16" width="0.4064" layer="94"/>
<text x="-5" y="0" size="1.27" layer="94">555</text>
<text x="-7.62" y="10.795" size="1.778" layer="95">&gt;NAME</text>
<text x="-7.62" y="-12.7" size="1.778" layer="96">&gt;VALUE</text>
<pin name="GND" x="-12.7" y="-7.62" visible="both" length="middle" direction="pwr"/>
<pin name="TRIG" x="-12.7" y="-2.54" visible="both" length="middle" direction="in"/>
<pin name="OUT" x="12.7" y="0" visible="both" length="middle" direction="out" rot="R180"/>
<pin name="RESET" x="-12.7" y="7.62" visible="both" length="middle" direction="in"/>
<pin name="CV" x="-12.7" y="2.54" visible="both" length="middle" direction="in"/>
<pin name="THR" x="12.7" y="-5.08" visible="both" length="middle" direction="in" rot="R180"/>
<pin name="DIS" x="12.7" y="5.08" visible="both" length="middle" direction="oc" rot="R180"/>
<pin name="VCC" x="12.7" y="7.62" visible="both" length="middle" direction="pwr" rot="R180"/>
</symbol>
<symbol name="CONN2">
<wire x1="-1.27" y1="-2.54" x2="3.81" y2="-2.54" width="0.4064" layer="94"/>
<wire x1="3.81" y1="-2.54" x2="3.81" y2="5.08" width="0.4064" layer="94"/>
<wire x1="3.81" y1="5.08" x2="-1.27" y2="5.08" width="0.4064" layer="94"/>
<wire x1="-1.27" y1="5.08" x2="-1.27" y2="-2.54" width="0.4064" layer="94"/>
<text x="-1.27" y="5.842" size="1.778" layer="95">&gt;NAME</text>
<text x="-1.27" y="-5.08" size="1.778" layer="96">&gt;VALUE</text>
<pin name="1" x="-5.08" y="0" visible="pad" length="middle" direction="pas"/>
<pin name="2" x="-5.08" y="2.54" visible="pad" length="middle" direction="pas"/>
</symbol>
<symbol name="GND">
<wire x1="-1.905" y1="0" x2="1.905" y2="0" width="0.254" layer="94"/>
<text x="-2.54" y="-2.54" size="1.778" layer="96">&gt;VALUE</text>
<pin name="GND" x="0" y="2.54" visible="off" length="short" direction="sup" rot="R270"/>
</symbol>
<symbol name="VCC">
<wire x1="1.27" y1="-1.905" x2="0" y2="0" width="0.254" layer="94"/>
<wire x1="0" y1="0" x2="-1.27" y2="-1.905" width="0.254" layer="94"/>
<text x="-2.54" y="-2.54" size="1.778" layer="96">&gt;VALUE</text>
<pin name="VCC" x="0" y="-2.54" visible="off" length="short" direction="sup" rot="R90"/>
</symbol>
</symbols>
<devicesets>
<deviceset name="R" prefix="R" uservalue="yes">
<description>Resistor</description>
<gates>
<gate name="G$1" symbol="R" x="0" y="0"/>
</gates>
<devices>
<device name="0805" package="R0805">
<connects><connect gate="G$1" pin="1" pad="1"/><connect gate="G$1" pin="2" pad="2"/></connects>
<technologies><technology name=""/></technologies>
</device>
</devices>
</deviceset>
<deviceset name="C" prefix="C" uservalue="yes">
<description>Capacitor</description>
<gates>
<gate name="G$1" symbol="C" x="0" y="0"/>
</gates>
<devices>
<device name="0603" package="C0603">
<connects><connect gate="G$1" pin="1" pad="1"/><connect gate="G$1" pin="2" pad="2"/></connects>
<technologies><technology name=""/></technologies>
</device>
</devices>
</deviceset>
<deviceset name="LED" prefix="D" uservalue="yes">
<description>Red LED, 3 mm</description>
<gates>
<gate name="G$1" symbol="LED" x="0" y="0"/>
</gates>
<devices>
<device name="" package="LED3MM">
<connects><connect gate="G$1" pin="A" pad="A"/><connect gate="G$1" pin="C" pad="C"/></connects>
<technologies><technology name=""/></technologies>
</device>
</devices>
</deviceset>
<deviceset name="NE555" prefix="U" uservalue="yes">
<description>Timer</description>
<gates>
<gate name="A" symbol="NE555" x="0" y="0"/>
</gates>
<devices>
<device name="D" package="SO08">
<connects><connect gate="A" pin="GND" pad="1"/><connect gate="A" pin="TRIG" pad="2"/><connect gate="A" pin="OUT" pad="3"/><connect gate="A" pin="RESET" pad="4"/><connect gate="A" pin="CV" pad="5"/><connect gate="A" pin="THR" pad="6"/><connect gate="A" pin="DIS" pad="7"/><connect gate="A" pin="VCC" pad="8"/></connects>
<technologies><technology name=""/></technologies>
</device>
</devices>
</deviceset>
<deviceset name="CONN2" prefix="J" uservalue="yes">
<description>Two pins</description>
<gates>
<gate name="G$1" symbol="CONN2" x="0" y="0"/>
</gates>
<devices>
<device name="" package="HDR2">
<connects><connect gate="G$1" pin="1" pad="1"/><connect gate="G$1" pin="2" pad="2"/></connects>
<technologies><technology name=""/></technologies>
</device>
</devices>
</deviceset>
<deviceset name="GND" uservalue="yes">
<description>Ground</description>
<gates>
<gate name="1" symbol="GND" x="0" y="0"/>
</gates>
<devices>
<device name="">
<connects></connects>
<technologies><technology name=""/></technologies>
</device>
</devices>
</deviceset>
<deviceset name="VCC" uservalue="yes">
<description>Supply</description>
<gates>
<gate name="1" symbol="VCC" x="0" y="0"/>
</gates>
<devices>
<device name="">
<connects></connects>
<technologies><technology name=""/></technologies>
</device>
</devices>
</deviceset>
</devicesets>
</library>
</libraries>
<attributes/>
<variantdefs/>
<classes>
<class number="0" name="default" width="0.254" drill="0.3" clearance="0.2"/>
<class number="1" name="power" width="0.5" drill="0.4" clearance="0.3"/>
</classes>
<parts>
<part name="U1" library="blinky" deviceset="NE555" device="D" value="NE555"><attribute name="MPN" value="NE555DR"/></part>
<part name="R1" library="blinky" deviceset="R" device="0805" value="10k"></part>
<part name="R2" library="blinky" deviceset="R" device="0805" value="47k"></part>
<part name="C1" library="blinky" deviceset="C" device="0603" value="10u"></part>
<part name="C2" library="blinky" deviceset="C" device="0603" value="10n"></part>
<part name="R3" library="blinky" deviceset="R" device="0805" value="330"></part>
<part name="D1" library="blinky" deviceset="LED" device="" value="red"></part>
<part name="J1" library="blinky" deviceset="CONN2" device="" value="POWER"></part>
<part name="GND1" library="blinky" deviceset="GND" device="" value="GND"></part>
<part name="GND2" library="blinky" deviceset="GND" device="" value="GND"></part>
<part name="VCC1" library="blinky" deviceset="VCC" device="" value="VCC"></part>
<part name="VCC2" library="blinky" deviceset="VCC" device="" value="VCC"></part>
</parts>
<sheets>
<sheet>
<description>Timer</description>
<plain>
<text x="20.32" y="180.34" size="2.54" layer="97">Timer (synthetic)</text>
<wire x1="15.24" y1="175.26" x2="80" y2="175.26" width="0.1524" layer="97" style="shortdash"/>
</plain>
<instances>
<instance part="U1" gate="A" x="101.6" y="101.6"/>
<instance part="R1" gate="G$1" x="60.96" y="121.92" rot="R90"/>
<instance part="R2" gate="G$1" x="60.96" y="86.36" rot="R270"/>
<instance part="C1" gate="G$1" x="40.64" y="71.12"/>
<instance part="C2" gate="G$1" x="152.4" y="71.12" rot="MR0"/>
<instance part="R3" gate="G$1" x="152.4" y="111.76" rot="MR90"/>
<instance part="D1" gate="G$1" x="172.72" y="91.44" rot="R180"/>
<instance part="GND1" gate="1" x="101.6" y="50.8"/>
<instance part="VCC1" gate="1" x="127" y="139.7"/>
</instances>
<busses/>
<nets>
<net name="VCC" class="1">
<segment>
<pinref part="U1" gate="A" pin="VCC"/>
<pinref part="U1" gate="A" pin="RESET"/>
<pinref part="R1" gate="G$1" pin="2"/>
<pinref part="VCC1" gate="1" pin="VCC"/>
<wire x1="114.3" y1="109.22" x2="115.57" y2="109.22" width="0.1524" layer="91"/>
<wire x1="115.57" y1="109.22" x2="115.57" y2="25.4" width="0.1524" layer="91"/>
<wire x1="88.9" y1="109.22" x2="87.63" y2="109.22" width="0.1524" layer="91"/>
<wire x1="87.63" y1="109.22" x2="87.63" y2="25.4" width="0.1524" layer="91"/>
<wire x1="60.96" y1="127" x2="62.23" y2="127" width="0.1524" layer="91"/>
<wire x1="62.23" y1="127" x2="62.23" y2="25.4" width="0.1524" layer="91"/>
<wire x1="127" y1="137.16" x2="128.27" y2="137.16" width="0.1524" layer="91"/>
<wire x1="128.27" y1="137.16" x2="128.27" y2="25.4" width="0.1524" layer="91"/>
<wire x1="62.23" y1="25.4" x2="128.27" y2="25.4" width="0.1524" layer="91"/>
<junction x="87.63" y="25.4"/>
<junction x="115.57" y="25.4"/>
<label x="63.5" y="25.9" size="1.778" layer="95"/>
</segment>
</net>
<net name="GND" class="1">
<segment>
<pinref part="U1" gate="A" pin="GND"/>
<pinref part="C1" gate="G$1" pin="2"/>
<pinref part="C2" gate="G$1" pin="2"/>
<pinref part="D1" gate="G$1" pin="C"/>
<pinref part="GND1" gate="1" pin="GND"/>
<wire x1="88.9" y1="93.98" x2="85.09" y2="93.98" width="0.1524" layer="91"/>
<wire x1="85.09" y1="93.98" x2="85.09" y2="22.86" width="0.1524" layer="91"/>
<wire x1="40.64" y1="66.04" x2="41.91" y2="66.04" width="0.1524" layer="91"/>
<wire x1="41.91" y1="66.04" x2="41.91" y2="22.86" width="0.1524" layer="91"/>
<wire x1="152.4" y1="66.04" x2="153.67" y2="66.04" width="0.1524" layer="91"/>
<wire x1="153.67" y1="66.04" x2="153.67" y2="22.86" width="0.1524" layer="91"/>
<wire x1="172.72" y1="96.52" x2="173.99" y2="96.52" width="0.1524" layer="91"/>
<wire x1="173.99" y1="96.52" x2="173.99" y2="22.86" width="0.1524" layer="91"/>
<wire x1="101.6" y1="53.34" x2="102.87" y2="53.34" width="0.1524" layer="91"/>
<wire x1="102.87" y1="53.34" x2="102.87" y2="22.86" width="0.1524" layer="91"/>
<wire x1="41.91" y1="22.86" x2="173.99" y2="22.86" width="0.1524" layer="91"/>
<junction x="85.09" y="22.86"/>
<junction x="102.87" y="22.86"/>
<junction x="153.67" y="22.86"/>
<label x="43.18" y="23.36" size="1.778" layer="95"/>
</segment>
</net>
<net name="N$1" class="0">
<segment>
<pinref part="R1" gate="G$1" pin="1"/>
<pinref part="R2" gate="G$1" pin="2"/>
<pinref part="U1" gate="A" pin="DIS"/>
<wire x1="60.96" y1="116.84" x2="64.77" y2="116.84" width="0.1524" layer="91"/>
<wire x1="64.77" y1="116.84" x2="64.77" y2="20.32" width="0.1524" layer="91"/>
<wire x1="60.96" y1="81.28" x2="67.31" y2="81.28" width="0.1524" layer="91"/>
<wire x1="67.31" y1="81.28" x2="67.31" y2="20.32" width="0.1524" layer="91"/>
<wire x1="114.3" y1="106.68" x2="118.11" y2="106.68" width="0.1524" layer="91"/>
<wire x1="118.11" y1="106.68" x2="118.11" y2="20.32" width="0.1524" layer="91"/>
<wire x1="64.77" y1="20.32" x2="118.11" y2="20.32" width="0.1524" layer="91"/>
<junction x="67.31" y="20.32"/>
<label x="66.04" y="20.82" size="1.778" layer="95"/>
</segment>
</net>
<net name="TIMING" class="0">
<segment>
<pinref part="R2" gate="G$1" pin="1"/>
<pinref part="C1" gate="G$1" pin="1"/>
<pinref part="U1" gate="A" pin="TRIG"/>
<pinref part="U1" gate="A" pin="THR"/>
<wire x1="60.96" y1="91.44" x2="69.85" y2="91.44" width="0.1524" layer="91"/>
<wire x1="69.85" y1="91.44" x2="69.85" y2="17.78" width="0.1524" layer="91"/>
<wire x1="40.64" y1="73.66" x2="44.45" y2="73.66" width="0.1524" layer="91"/>
<wire x1="44.45" y1="73.66" x2="44.45" y2="17.78" width="0.1524" layer="91"/>
<wire x1="88.9" y1="99.06" x2="82.55" y2="99.06" width="0.1524" layer="91"/>
<wire x1="82.55" y1="99.06" x2="82.55" y2="17.78" width="0.1524" layer="91"/>
<wire x1="114.3" y1="96.52" x2="120.65" y2="96.52" width="0.1524" layer="91"/>
<wire x1="120.65" y1="96.52" x2="120.65" y2="17.78" width="0.1524" layer="91"/>
<wire x1="44.45" y1="17.78" x2="120.65" y2="17.78" width="0.1524" layer="91"/>
<junction x="69.85" y="17.78"/>
<junction x="82.55" y="17.78"/>
<label x="45.72" y="18.28" size="1.778" layer="95"/>
</segment>
</net>
<net name="N$2" class="0">
<segment>
<pinref part="U1" gate="A" pin="CV"/>
<pinref part="C2" gate="G$1" pin="1"/>
<wire x1="88.9" y1="104.14" x2="80.01" y2="104.14" width="0.1524" layer="91"/>
<wire x1="80.01" y1="104.14" x2="80.01" y2="15.24" width="0.1524" layer="91"/>
<wire x1="152.4" y1="73.66" x2="156.21" y2="73.66" width="0.1524" layer="91"/>
<wire x1="156.21" y1="73.66" x2="156.21" y2="15.24" width="0.1524" layer="91"/>
<wire x1="80.01" y1="15.24" x2="156.21" y2="15.24" width="0.1524" layer="91"/>
<label x="81.28" y="15.74" size="1.778" layer="95"/>
</segment>
</net>
<net name="N$3" class="0">
<segment>
<pinref part="U1" gate="A" pin="OUT"/>
<pinref part="R3" gate="G$1" pin="1"/>
<wire x1="114.3" y1="101.6" x2="123.19" y2="101.6" width="0.1524" layer="91"/>
<wire x1="123.19" y1="101.6" x2="123.19" y2="12.7" width="0.1524" layer="91"/>
<wire x1="152.4" y1="106.68" x2="158.75" y2="106.68" width="0.1524" layer="91"/>
<wire x1="158.75" y1="106.68" x2="158.75" y2="12.7" width="0.1524" layer="91"/>
<wire x1="123.19" y1="12.7" x2="158.75" y2="12.7" width="0.1524" layer="91"/>
<label x="124.46" y="13.2" size="1.778" layer="95"/>
</segment>
</net>
<net name="LED" class="0">
<segment>
<pinref part="R3" gate="G$1" pin="2"/>
<pinref part="D1" gate="G$1" pin="A"/>
<wire x1="152.4" y1="116.84" x2="161.29" y2="116.84" width="0.1524" layer="91"/>
<wire x1="161.29" y1="116.84" x2="161.29" y2="10.16" width="0.1524" layer="91"/>
<wire x1="172.72" y1="88.9" x2="176.53" y2="88.9" width="0.1524" layer="91"/>
<wire x1="176.53" y1="88.9" x2="176.53" y2="10.16" width="0.1524" layer="91"/>
<wire x1="161.29" y1="10.16" x2="176.53" y2="10.16" width="0.1524" layer="91"/>
<label x="162.56" y="10.66" size="1.778" layer="95"/>
</segment>
</net>
</nets>
</sheet>
<sheet>
<description>Power input</description>
<plain>
<text x="20.32" y="180.34" size="2.54" layer="97">Power input (synthetic)</text>
<wire x1="15.24" y1="175.26" x2="80" y2="175.26" width="0.1524" layer="97" style="shortdash"/>
</plain>
<instances>
<instance part="J1" gate="G$1" x="50.8" y="101.6"/>
<instance part="GND2" gate="1" x="76.2" y="76.2"/>
<instance part="VCC2" gate="1" x="76.2" y="127"/>
</instances>
<busses/>
<nets>
<net name="VCC" class="1">
<segment>
<pinref part="J1" gate="G$1" pin="2"/>
<pinref part="VCC2" gate="1" pin="VCC"/>
<wire x1="45.72" y1="104.14" x2="44.45" y2="104.14" width="0.1524" layer="91"/>
<wire x1="44.45" y1="104.14" x2="44.45" y2="25.4" width="0.1524" layer="91"/>
<wire x1="76.2" y1="124.46" x2="77.47" y2="124.46" width="0.1524" layer="91"/>
<wire x1="77.47" y1="124.46" x2="77.47" y2="25.4" width="0.1524" layer="91"/>
<wire x1="44.45" y1="25.4" x2="77.47" y2="25.4" width="0.1524" layer="91"/>
<label x="45.72" y="25.9" size="1.778" layer="95"/>
</segment>
</net>
<net name="GND" class="1">
<segment>
<pinref part="J1" gate="G$1" pin="1"/>
<pinref part="GND2" gate="1" pin="GND"/>
<wire x1="45.72" y1="101.6" x2="41.91" y2="101.6" width="0.1524" layer="91"/>
<wire x1="41.91" y1="101.6" x2="41.91" y2="22.86" width="0.1524" layer="91"/>
<wire x1="76.2" y1="78.74" x2="80.01" y2="78.74" width="0.1524" layer="91"/>
<wire x1="80.01" y1="78.74" x2="80.01" y2="22.86" width="0.1524" layer="91"/>
<wire x1="41.91" y1="22.86" x2="80.01" y2="22.86" width="0.1524" layer="91"/>
<label x="43.18" y="23.36" size="1.778" layer="95"/>
</segment>
</net>
</nets>
</sheet>
</sheets>
</schematic>
</drawing>
</eagle>
