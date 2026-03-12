// FILE: INIControlBarScheme.cpp /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Apr 2002
//
//	Filename: 	INIControlBarScheme.cpp
//
//	author:		Chris Huybregts
//	
//	purpose:	Parse a control Bar Scheme
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

//-----------------------------------------------------------------------------
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "GameClient/ControlBar.h"
#include "GameClient/ControlBarScheme.h"
//-----------------------------------------------------------------------------
// DEFINES ////////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-----------------------------------------------------------------------------
// PRIVATE FUNCTIONS //////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
/** Parse a ControlBarScheme button */
//-------------------------------------------------------------------------------------------------
void INI::parseControlBarSchemeDefinition( INI *ini )
{
	AsciiString name;
	ControlBarSchemeManager *CBSchemeManager;
	ControlBarScheme *CBScheme;

	// read the name
	const char* c = ini->getNextToken();
	name.set( c );	

	// find existing item if present
	CBSchemeManager = TheControlBar->getControlBarSchemeManager();
	DEBUG_ASSERTCRASH( CBSchemeManager, ("parseControlBarSchemeDefinition: Unable to Get CBSchemeManager\n") );
	if( !CBSchemeManager )
		return;

	// If we have a previously allocated control bar, this will return a cleared out pointer to it so we
	// can overwrite it	
	CBScheme = CBSchemeManager->newControlBarScheme( name );

	// sanity
	DEBUG_ASSERTCRASH( CBScheme, ("parseControlBarSchemeDefinition: Unable to allocate Scheme '%s'\n", name.str()) );

	// parse the ini definition
	ini->initFromINI( CBScheme, CBSchemeManager->getFieldParse() );

}  // end parseCommandButtonDefinition


