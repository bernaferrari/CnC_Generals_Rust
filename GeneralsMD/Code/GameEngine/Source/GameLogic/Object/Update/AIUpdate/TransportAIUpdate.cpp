// TransportAIUpdate.cpp //////////
// Needs to check legality of evacuate, and may move to a place that is better to evacuate at 
// Author: Graham Smallwood, July 2002

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/RandomValue.h"
#include "GameLogic/Module/TransportAIUpdate.h"
#include "GameLogic/Module/BehaviorModule.h"
#include "GameLogic/Module/ContainModule.h"
#include "GameLogic/Object.h"



#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif


//-------------------------------------------------------------------------------------------------
AIStateMachine* TransportAIUpdate::makeStateMachine()
{
	return newInstance(AIStateMachine)( getObject(), "TransportAIUpdateMachine");
}

//-------------------------------------------------------------------------------------------------
TransportAIUpdate::TransportAIUpdate( Thing *thing, const ModuleData* moduleData ) : AIUpdateInterface( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
TransportAIUpdate::~TransportAIUpdate( void )
{

}

//-------------------------------------------------------------------------------------------------
/**
 * Attack given object
 */
void TransportAIUpdate::privateAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource )
{
	ContainModuleInterface* contain = getObject()->getContain();
	if( contain != NULL  &&  contain->isPassengerAllowedToFire() )
	{
		// As an extension of the normal attack, I may want to tell my passengers to attack 
		// too, but only if this is a direct command.  (As opposed to a passive aquire)
		if( cmdSource == CMD_FROM_PLAYER  ||  cmdSource == CMD_FROM_SCRIPT )
		{
			const ContainedItemsList *passengerList = contain->getContainedItemsList();
			ContainedItemsList::const_iterator passengerIterator;
			passengerIterator = passengerList->begin();

			while( passengerIterator != passengerList->end() )
			{
				Object *passenger = *passengerIterator;
				//Advance to the next iterator
				passengerIterator++;

				// If I am an overlord with a gattling upgrade, I do not tell it to fire if it is disabled
				if ( passenger->isKindOf( KINDOF_PORTABLE_STRUCTURE ) )
				{
					if( passenger->isDisabledByType( DISABLED_HACKED ) 
						|| passenger->isDisabledByType( DISABLED_EMP ) 
						|| passenger->isDisabledByType( DISABLED_SUBDUED ) 
						|| passenger->isDisabledByType( DISABLED_PARALYZED) )
						continue;
				}
				
				AIUpdateInterface *passengerAI = passenger->getAIUpdateInterface();
				if( passengerAI )
				{
					passengerAI->aiAttackObject( victim, maxShotsToFire, cmdSource );
				}
			}
		}
	}

	AIUpdateInterface::privateAttackObject( victim, maxShotsToFire, cmdSource );
}

//-------------------------------------------------------------------------------------------------
/**
 * Attack given object
 */
void TransportAIUpdate::privateForceAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource )
{
	ContainModuleInterface* contain = getObject()->getContain();
	if( contain != NULL  &&  contain->isPassengerAllowedToFire() )
	{
		// As an extension of the normal attack, I may want to tell my passengers to attack 
		// too, but only if this is a direct command.  (As opposed to a passive aquire)
		if( cmdSource == CMD_FROM_PLAYER  ||  cmdSource == CMD_FROM_SCRIPT )
		{
			const ContainedItemsList *passengerList = contain->getContainedItemsList();
			ContainedItemsList::const_iterator passengerIterator;
			passengerIterator = passengerList->begin();

			while( passengerIterator != passengerList->end() )
			{
				Object *passenger = *passengerIterator;
				//Advance to the next iterator
				passengerIterator++;

				// If I am an overlord with a gattling upgrade, I do not tell it to fire if it is disabled
				if ( passenger->isKindOf( KINDOF_PORTABLE_STRUCTURE ) )
				{
					if( passenger->isDisabledByType( DISABLED_HACKED ) 
						|| passenger->isDisabledByType( DISABLED_EMP ) 
						|| passenger->isDisabledByType( DISABLED_SUBDUED ) 
						|| passenger->isDisabledByType( DISABLED_PARALYZED) )
						continue;
				}
				
				AIUpdateInterface *passengerAI = passenger->getAIUpdateInterface();
				if( passengerAI )
				{
					passengerAI->aiForceAttackObject( victim, maxShotsToFire, cmdSource );
				}
			}
		}
	}

	AIUpdateInterface::privateForceAttackObject( victim, maxShotsToFire, cmdSource );
}

//-------------------------------------------------------------------------------------------------
/**
 * Attack given position
 */
void TransportAIUpdate::privateAttackPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource )
{
	ContainModuleInterface* contain = getObject()->getContain();
	if( contain != NULL  &&  contain->isPassengerAllowedToFire() )
	{
		// As an extension of the normal attack, I may want to tell my passengers to attack 
		// too, but only if this is a direct command.  (As opposed to a passive aquire)
		if( cmdSource == CMD_FROM_PLAYER  ||  cmdSource == CMD_FROM_SCRIPT )
		{
			const ContainedItemsList *passengerList = contain->getContainedItemsList();
			ContainedItemsList::const_iterator passengerIterator;
			passengerIterator = passengerList->begin();

			while( passengerIterator != passengerList->end() )
			{
				Object *passenger = *passengerIterator;
				//Advance to the next iterator
				passengerIterator++;

				// If I am an overlord with a gattling upgrade, I do not tell it ti fire if it is disabled
				if ( passenger->isKindOf( KINDOF_PORTABLE_STRUCTURE ) )
				{
					if( passenger->isDisabledByType( DISABLED_HACKED ) 
						|| passenger->isDisabledByType( DISABLED_EMP) 
						|| passenger->isDisabledByType( DISABLED_SUBDUED ) 
						|| passenger->isDisabledByType( DISABLED_PARALYZED) )
						continue;
				}

				AIUpdateInterface *passengerAI = passenger->getAIUpdateInterface();
				if( passengerAI )
				{
					passengerAI->aiAttackPosition( pos, maxShotsToFire, cmdSource );
				}
			}
		}
	}

	AIUpdateInterface::privateAttackPosition( pos, maxShotsToFire, cmdSource );
}

//-------------------------------------------------------------------------------------------------
AIFreeToExitType TransportAIUpdate::getAiFreeToExit(const Object* exiter) const 
{ 
	// Transports have a speed at which you can exit.
	return FREE_TO_EXIT; 
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void TransportAIUpdate::crc( Xfer *xfer )
{
	// extend base class
	AIUpdateInterface::crc(xfer);
}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void TransportAIUpdate::xfer( Xfer *xfer )
{
  XferVersion currentVersion = 1;
  XferVersion version = currentVersion;
  xfer->xferVersion( &version, currentVersion );
 
 // extend base class
	AIUpdateInterface::xfer(xfer);

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void TransportAIUpdate::loadPostProcess( void )
{
 // extend base class
	AIUpdateInterface::loadPostProcess();
}  // end loadPostProcess
