// FILE: INIAnimation.cpp /////////////////////////////////////////////////////////////////////////
// Author: Colin Day, July 2002
// Desc:   Parsing animation INI entries for 2D image animations
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "GameClient/Anim2D.h"

//-------------------------------------------------------------------------------------------------
/** Parse animation entry */
//-------------------------------------------------------------------------------------------------
void INI::parseAnim2DDefinition( INI* ini )
{
	AsciiString name;
	Anim2DTemplate *animTemplate;

	// read the name
	const char* c = ini->getNextToken();
	name.set( c );	

	//
	// find existing item if present, note that we do not support overrides
	// in the animations like we do in systems that are more "design" oriented, images
	// are assets as they are
	//
	if( !TheAnim2DCollection )
	{

		//We don't need it if we're in the builder... which doesn't have this.
		return;

	}  // end if

	// find existing animation template if present
	animTemplate = TheAnim2DCollection->findTemplate( name );
	if( animTemplate == NULL )
	{

		// item not found, create a new one
		animTemplate = TheAnim2DCollection->newTemplate( name );
		DEBUG_ASSERTCRASH( animTemplate, ("INI""parseAnim2DDefinition -  unable to allocate animation template for '%s'\n",
											 name.str()) );

	}  // end if
	else
	{

		// we're loading over an existing animation template ... something is probably wrong
		DEBUG_CRASH(( "INI::parseAnim2DDefinition - Animation template '%s' already exists\n",
									animTemplate->getName().str() ));
		return;

	}  // end else

	// parse the ini definition
	ini->initFromINI( animTemplate, animTemplate->getFieldParse() );

}  // end parseAnim2DDefinition



