// FILE: Money.h ////////////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  Money.h
//
// Created:    Steven Johnson, October 2001
//
// Desc:			 @todo
//
//-----------------------------------------------------------------------------

#pragma once

#ifndef _MONEY_H_
#define _MONEY_H_

#include "Lib/BaseType.h"
#include "Common/Debug.h"
#include "Common/Snapshot.h"

// ----------------------------------------------------------------------------------------------
/**
	How much "money" (Tiberium, Gems, Magic Resource Boxes, whatever) the Player has.
	This is currently a Very Simple Class but is encapsulated
	in anticipation of future expansion.
*/
class Money : public Snapshot
{

public:

	inline Money() : m_money(0), m_playerIndex(0)
	{
	}

	void init()
	{
		m_money = 0;
	}

	inline UnsignedInt countMoney() const 
	{ 
		return m_money; 
	}

	/// returns the actual amount withdrawn, which may be less than you want. (sorry, can't go into debt...)
	UnsignedInt withdraw(UnsignedInt amountToWithdraw, Bool playSound = TRUE);
	void deposit(UnsignedInt amountToDeposit, Bool playSound = TRUE);

	void setPlayerIndex(Int ndx) { m_playerIndex = ndx; }
	
  static void parseMoneyAmount( INI *ini, void *instance, void *store, const void* userData );

  // Does the amount of this == the amount of that (compare everything except m_playerIndex)
  Bool amountEqual( const Money & that ) const
  {
    return m_money == that.m_money;
  }

protected:
	// snapshot methods
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

private:

	UnsignedInt m_money;	///< amount of money
	Int m_playerIndex;	///< what is my player index?
};

#endif // _MONEY_H_

