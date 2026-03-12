// FILE: ControlBarResizer.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Sep 2002
//
//	Filename: 	ControlBarResizer.h
//
//	author:		Chris Huybregts
//	
//	purpose:	
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __CONTROL_BAR_RESIZER_H_
#define __CONTROL_BAR_RESIZER_H_

//-----------------------------------------------------------------------------
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// FORWARD REFERENCES /////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// TYPE DEFINES ///////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class ResizerWindow
{
public:
ResizerWindow( void );
	AsciiString m_name;
	ICoord2D m_defaultSize;
	ICoord2D m_defaultPos;
	ICoord2D m_altSize;
	ICoord2D m_altPos;
};

class ControlBarResizer
{
public:
	ControlBarResizer( void );
	~ControlBarResizer( void );
	
	void init( void );

	// parse Functions for the INI file
	const FieldParse *getFieldParse() const { return m_controlBarResizerParseTable; }								///< returns the parsing fields
	static const FieldParse m_controlBarResizerParseTable[];																				///< the parse table

	ResizerWindow *findResizerWindow( AsciiString name ); ///< attempt to find the control bar scheme by it's name
	ResizerWindow *newResizerWindow( AsciiString name );	///< create a new control bar scheme and return it.
	
	void sizeWindowsDefault( void );
	void sizeWindowsAlt( void );

	typedef std::list< ResizerWindow *> ResizerWindowList;
	ResizerWindowList m_resizerWindowsList;

};
//-----------------------------------------------------------------------------
// INLINING ///////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// EXTERNALS //////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

#endif // __CONTROL_BAR_RESIZER_H_
