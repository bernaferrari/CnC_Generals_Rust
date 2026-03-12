// FILE: DrawGroupInfo.h //////////////////////////////////////////////////////////////////////////
// AudioEventRTS structure
// Author: John K. McDonald, March 2002

#pragma once
#ifndef _H_DRAWGROUPINFO_
#define _H_DRAWGROUPINFO_

struct DrawGroupInfo
{
	AsciiString m_fontName;
	Int m_fontSize;
	Bool m_fontIsBold;

	Bool m_usePlayerColor;
	Color m_colorForText;
	Color m_colorForTextDropShadow;

	Int m_dropShadowOffsetX;
	Int m_dropShadowOffsetY;

	union 
	{
		Int m_pixelOffsetX;
		Real m_percentOffsetX;
	};
	Bool m_usingPixelOffsetX;

	union 
	{
		Int m_pixelOffsetY;
		Real m_percentOffsetY;
	};
	Bool m_usingPixelOffsetY;

	DrawGroupInfo();
	
	static const FieldParse s_fieldParseTable[];		///< the parse table for INI definition
	const FieldParse *getFieldParse( void ) const { return s_fieldParseTable; }
};

extern DrawGroupInfo *TheDrawGroupInfo;

#endif /* _H_DRAWGROUPINFO */