// FILE: INIMultiplayer.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, January 2002
// Desc:   Parsing MultiplayerSettings and MultiplayerColor INI entries
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "Common/MultiplayerSettings.h"

void INI::parseMultiplayerSettingsDefinition( INI* ini )
{
	if( TheMultiplayerSettings )
	{
		// 
		// if the type of loading we're doing creates override data, we need to
		// be loading into a new override item
		//
		if( ini->getLoadType() == INI_LOAD_CREATE_OVERRIDES )
		{
			DEBUG_ASSERTCRASH(false, ("Creating an override of MultiplayerSettings!"));
		}
	}  // end if
	else
	{
		// we don't have any multiplayer settings instance at all yet, create one
		TheMultiplayerSettings = NEW MultiplayerSettings;
	}  // end else

	// parse the ini definition
	ini->initFromINI( TheMultiplayerSettings, TheMultiplayerSettings->getFieldParse() );
}

void INI::parseMultiplayerColorDefinition( INI* ini )
{
	const char *c;
	AsciiString name;
	MultiplayerColorDefinition *multiplayerColorDefinition;

	// read the name
	c = ini->getNextToken();
	name.set( c );	

	// find existing item if present, but this type does not allow overrides, 
	//so if it exists just overwrite it.
	multiplayerColorDefinition = TheMultiplayerSettings->findMultiplayerColorDefinitionByName( name );
	if( multiplayerColorDefinition == NULL )
		multiplayerColorDefinition = TheMultiplayerSettings->newMultiplayerColorDefinition( name );

	ini->initFromINI( multiplayerColorDefinition, multiplayerColorDefinition->getFieldParse() );

	multiplayerColorDefinition->setColor(multiplayerColorDefinition->getRGBValue());
	multiplayerColorDefinition->setNightColor(multiplayerColorDefinition->getRGBNightValue());
}

namespace
{
  struct MultiplayerStartingMoneySettings
  {
    Money money;
    Bool  isDefault;
  };
  
  const FieldParse startingMoneyFieldParseTable[] = 
  {
    { "Value",			  Money::parseMoneyAmount,	NULL,	offsetof( MultiplayerStartingMoneySettings, money ) },
    { "Default",	   	INI::parseBool,         	NULL,	offsetof( MultiplayerStartingMoneySettings, isDefault ) },
    { NULL,	NULL,	NULL,	0 }  // keep this last
  };
}


void INI::parseMultiplayerStartingMoneyChoiceDefinition( INI* ini )
{
  DEBUG_ASSERTCRASH( ini->getLoadType() != INI_LOAD_CREATE_OVERRIDES, ("Overrides not supported for MultiplayerStartingMoneyChoice") );
  
  // Temporary data store
  MultiplayerStartingMoneySettings settings;
  settings.isDefault = false;
  
  ini->initFromINI( &settings, startingMoneyFieldParseTable );
  
  TheMultiplayerSettings->addStartingMoneyChoice( settings.money, settings.isDefault );
}
