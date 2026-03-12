// FILE: GameSpyThread.h //////////////////////////////////////////////////////
// Generals GameSpy thread class
// Author: Matthew D. Campbell, March 2002

#pragma once

#ifndef __GAMESPYTHREAD_H__
#define __GAMESPYTHREAD_H__

#include "mutex.h"
#include "thread.h"

class GameSpyThreadClass : public ThreadClass
{

public:
	GameSpyThreadClass::GameSpyThreadClass() : ThreadClass() { m_doLogin = false; m_readStats = false; m_updateWins = false; m_updateLosses = false; m_updateLocale = false; m_showLocaleSelect = false; m_nextShellScreen.clear(); }
	void queueLogin(AsciiString nick, AsciiString pass, AsciiString email) { m_nick = nick; m_pass = pass; m_email = email; m_doLogin = true; }
	void queueReadPersistentStatsFromServer( void ) { m_readStats = true; }
	void queueUpdateLocale( AsciiString locale ) { m_locale = locale; m_updateLocale = true; }
	void queueUpdateWins  ( AsciiString wins   ) { m_wins   = wins;   m_updateWins   = true; }
	void queueUpdateLosses( AsciiString losses ) { m_losses = losses; m_updateLosses = true; }

	void Thread_Function();

	AsciiString getNextShellScreen( void );
	Bool showLocaleSelect( void );

	void setNextShellScreen( AsciiString nextShellScreen );
	void setShowLocaleSelect( Bool val );

private:
	AsciiString m_nick, m_pass, m_email;
	Bool m_doLogin, m_readStats, m_updateWins, m_updateLosses, m_updateLocale;
	AsciiString m_locale, m_wins, m_losses;
	AsciiString m_nextShellScreen;
	Bool m_showLocaleSelect;
};

extern GameSpyThreadClass *TheGameSpyThread;
extern MutexClass TheGameSpyMutex;

#endif // __GAMESPYTHREAD_H__
