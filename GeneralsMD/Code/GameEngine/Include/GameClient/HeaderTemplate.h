// FILE: HeaderTemplate.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Aug 2002
//
//	Filename: 	HeaderTemplate.h
//
//	author:		Chris Huybregts
//	
//	purpose:	
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __HEADER_TEMPLATE_H_
#define __HEADER_TEMPLATE_H_

//-----------------------------------------------------------------------------
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "Common/STLTypedefs.h"
//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "Common/AsciiString.h"
//-----------------------------------------------------------------------------
// FORWARD REFERENCES /////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class GameFont;
struct FieldParse;
class INI;
//-----------------------------------------------------------------------------
// TYPE DEFINES ///////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class HeaderTemplate
{
public:
	HeaderTemplate( void );
	~HeaderTemplate( void );

	GameFont *m_font;
	AsciiString m_name;
	AsciiString m_fontName;
	Int m_point;
	Bool m_bold;
	
};

class HeaderTemplateManager
{
public:
	HeaderTemplateManager( void );
	~HeaderTemplateManager( void );	

	void init( void );
	
	const FieldParse *getFieldParse( void ) const { return m_headerFieldParseTable; }		///< Return the field parse info
	static const FieldParse m_headerFieldParseTable[];
	
	HeaderTemplate *findHeaderTemplate( AsciiString name );
	HeaderTemplate *newHeaderTemplate( AsciiString name );
	
	GameFont *getFontFromTemplate( AsciiString name );
	HeaderTemplate *getFirstHeader( void );
	HeaderTemplate *getNextHeader( HeaderTemplate *ht );

	void headerNotifyResolutionChange(void);

private:
	void populateGameFonts( void );
	typedef std::list< HeaderTemplate* > HeaderTemplateList;
	typedef HeaderTemplateList::iterator HeaderTemplateListIt;
	HeaderTemplateList m_headerTemplateList;

};


//-----------------------------------------------------------------------------
// INLINING ///////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// EXTERNALS //////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
extern HeaderTemplateManager *TheHeaderTemplateManager;
#endif // __HEADER_TEMPLATE_H_
