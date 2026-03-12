// FILE: INIParticleSys.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Michael S. Booth, November 2001
// Desc:   Parsing Particle System INI entries
///////////////////////////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "GameClient/ParticleSys.h"


/** 
 * Parse entry 
 */
void INI::parseParticleSystemDefinition( INI* ini )
{
	AsciiString name;

	// read the name
	const char* c = ini->getNextToken();
	name.set( c );	

	// find existing item if present
	ParticleSystemTemplate *sysTemplate = const_cast<ParticleSystemTemplate*>(TheParticleSystemManager->findTemplate( name ));
	if (sysTemplate == NULL)
	{
		// no item is present, create a new one
		sysTemplate = TheParticleSystemManager->newTemplate( name );
	}

	// parse the ini definition
	ini->initFromINI( sysTemplate, sysTemplate->getFieldParse() );
}
