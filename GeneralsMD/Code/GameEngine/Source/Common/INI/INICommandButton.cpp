// FILE: INICommandButton.cpp /////////////////////////////////////////////////////////////////////
// Author: Colin Day, March 2002
// Desc:   Command buttons are the atomic units we can configure into command sets to then
//				 display in the context sensitive user interface
///////////////////////////////////////////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "Common/SpecialPower.h"
#include "GameClient/ControlBar.h"

//-------------------------------------------------------------------------------------------------
/** Parse a command button */
//-------------------------------------------------------------------------------------------------
void INI::parseCommandButtonDefinition( INI *ini )
{
	ControlBar::parseCommandButtonDefinition(ini);
}

//-------------------------------------------------------------------------------------------------
/** Parse a command button */
//-------------------------------------------------------------------------------------------------
void ControlBar::parseCommandButtonDefinition( INI *ini )
{
	// read the name
	AsciiString name = ini->getNextToken();

	// find existing item if present
	CommandButton *button = TheControlBar->findNonConstCommandButton( name );
	if( button == NULL )
	{
		// allocate a new item
		button = TheControlBar->newCommandButton( name );
		if (ini->getLoadType() == INI_LOAD_CREATE_OVERRIDES) 
		{
			button->markAsOverride();
		}
	}  // end if
	else if( ini->getLoadType() != INI_LOAD_CREATE_OVERRIDES )
	{
		DEBUG_CRASH(( "[LINE: %d in '%s'] Duplicate commandbutton %s found!", ini->getLineNum(), ini->getFilename().str(), name.str() ));
	}
	else
	{
		button = TheControlBar->newCommandButtonOverride( button );
	}

	// parse the ini definition
	ini->initFromINI( button, button->getFieldParse() );
	

	//Make sure buttons with special power templates also have the appropriate option set.
	const SpecialPowerTemplate *spTemplate = button->getSpecialPowerTemplate();
	Bool needsTemplate = BitTest( button->getOptions(), NEED_SPECIAL_POWER_SCIENCE );
	if( spTemplate && !needsTemplate )
	{
		DEBUG_CRASH( ("[LINE: %d in '%s'] CommandButton %s has SpecialPower = %s but the button also requires Options = NEED_SPECIAL_POWER_SCIENCE. Failure to do so will cause bugs such as invisible side shortcut buttons",
			ini->getLineNum(), ini->getFilename().str(), name.str(), spTemplate->getName().str() ) );
	}
	else if( !spTemplate && needsTemplate )
	{
		DEBUG_CRASH( ("[LINE: %d in '%s'] CommandButton %s has Options = NEED_SPECIAL_POWER_SCIENCE but doesn't specify a SpecialPower = xxxx. Please evaluate INI.",
			ini->getLineNum(), ini->getFilename().str(), name.str() ) );
	}

}  // end parseCommandButtonDefinition


