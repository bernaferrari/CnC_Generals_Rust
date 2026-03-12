// FILE: Money.cpp /////////////////////////////////////////////////////////
//
// Project:   RTS3
//
// File name: Money.cpp
//
// Created:   Steven Johnson, October 2001
//
// Desc:      @todo
//
//-----------------------------------------------------------------------------

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/Money.h"

#include "Common/GameAudio.h"
#include "Common/MiscAudio.h"
#include "Common/Player.h"
#include "Common/PlayerList.h"
#include "Common/Xfer.h"

// ------------------------------------------------------------------------------------------------
UnsignedInt Money::withdraw(UnsignedInt amountToWithdraw, Bool playSound)
{
	if (amountToWithdraw > m_money)
		amountToWithdraw = m_money;

	if (amountToWithdraw == 0)
		return amountToWithdraw;

	//@todo: Do we do this frequently enough that it is a performance hit?
	AudioEventRTS event = TheAudio->getMiscAudio()->m_moneyWithdrawSound;
	event.setPlayerIndex(m_playerIndex);

	// Play a sound
	if (playSound)
		TheAudio->addAudioEvent(&event);

	m_money -= amountToWithdraw;

	return amountToWithdraw;
}

// ------------------------------------------------------------------------------------------------
void Money::deposit(UnsignedInt amountToDeposit, Bool playSound)
{
	if (amountToDeposit == 0)
		return;

	//@todo: Do we do this frequently enough that it is a performance hit?
	AudioEventRTS event = TheAudio->getMiscAudio()->m_moneyDepositSound;
	event.setPlayerIndex(m_playerIndex);

	// Play a sound
	if (playSound)
		TheAudio->addAudioEvent(&event);
	
	m_money += amountToDeposit;

	if( amountToDeposit > 0 )
	{
		Player *player = ThePlayerList->getNthPlayer( m_playerIndex );
		if( player )
		{
			player->getAcademyStats()->recordIncome();
		}
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void Money::crc( Xfer *xfer )
{

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void Money::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// money value
	xfer->xferUnsignedInt( &m_money );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void Money::loadPostProcess( void )
{

}  // end loadPostProcess


// ------------------------------------------------------------------------------------------------
/** Parse a money amount for the ini file. E.g. DefaultStartingMoney = 10000 */
// ------------------------------------------------------------------------------------------------
void Money::parseMoneyAmount( INI *ini, void *instance, void *store, const void* userData )
{
  // Someday, maybe, have mulitple fields like Gold:10000 Wood:1000 Tiberian:10
  Money * money = (Money *)store;
  INI::parseUnsignedInt( ini, instance, &money->m_money, userData );
}
