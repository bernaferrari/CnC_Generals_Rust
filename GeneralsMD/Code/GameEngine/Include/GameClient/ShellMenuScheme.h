// FILE: ShellMenuScheme.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Jul 2002
//
//	Filename: 	ShellMenuScheme.h
//
//	author:		Chris Huybregts
//	
//	purpose:	
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SHELL_MENU_SCHEME_H_
#define __SHELL_MENU_SCHEME_H_

//-----------------------------------------------------------------------------
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "GameClient/Color.h"

//-----------------------------------------------------------------------------
// FORWARD REFERENCES /////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class Image;

//-----------------------------------------------------------------------------
// TYPE DEFINES ///////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class ShellMenuSchemeLine
{
public:
	ShellMenuSchemeLine( void );
	~ShellMenuSchemeLine( void );	
	
	ICoord2D m_startPos;
	ICoord2D m_endPos;
	Int m_width;
	Color m_color;

};

class ShellMenuSchemeImage
{
public:
	ShellMenuSchemeImage( void );
	~ShellMenuSchemeImage( void );	

	AsciiString m_name;						///< Name of the image
	ICoord2D m_position;					///< the position we'll draw it at
	ICoord2D m_size;							///< the size of the image needed when we draw it
	Image *m_image;								///< the actual pointer to the mapped image
};

class ShellMenuScheme
{
public:
	ShellMenuScheme( void );
	~ShellMenuScheme( void );	
	
	void draw( void );
	void addImage( ShellMenuSchemeImage* schemeImage );
	void addLine( ShellMenuSchemeLine* schemeLine );
	

	AsciiString m_name;

	typedef std::list< ShellMenuSchemeImage* > ShellMenuSchemeImageList;
	typedef ShellMenuSchemeImageList::iterator ShellMenuSchemeImageListIt;
	ShellMenuSchemeImageList m_imageList;
	
	typedef std::list< ShellMenuSchemeLine* > ShellMenuSchemeLineList;
	typedef ShellMenuSchemeLineList::iterator ShellMenuSchemeLineListIt;
	ShellMenuSchemeLineList m_lineList;


	

};

class ShellMenuSchemeManager
{
public:
	ShellMenuSchemeManager( void );
	~ShellMenuSchemeManager( void );
	
	void init( void );
	void update( void );

	void setShellMenuScheme( AsciiString name );
	
	void draw( void );

	// parse Functions for the INI file
	const FieldParse *getFieldParse() const { return m_shellMenuSchemeFieldParseTable; }								///< returns the parsing fields
	static const FieldParse m_shellMenuSchemeFieldParseTable[];																				///< the parse table
	static void parseImagePart( INI* ini, void *instance, void *store, const void *userData );					///< Parse the image part of the INI file
	static void parseLinePart( INI* ini, void *instance, void *store, const void *userData );					///< Parse the line part of the INI file

	ShellMenuScheme *newShellMenuScheme(AsciiString name);

private:
	typedef std::list< ShellMenuScheme* > ShellMenuSchemeList;			///< list of Shell Menu schemes
	typedef ShellMenuSchemeList::iterator ShellMenuSchemeListIt;
	ShellMenuSchemeList m_schemeList;
	ShellMenuScheme *m_currentScheme;

};


//-----------------------------------------------------------------------------
// INLINING ///////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// EXTERNALS //////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

#endif // __SHELL_MENU_SCHEME_H_
