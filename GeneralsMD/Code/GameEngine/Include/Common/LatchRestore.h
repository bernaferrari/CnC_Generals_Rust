// LatchRestore.h /////////////////////////////////////////////////////////////////////////////////
// Author: John K. McDonald, Jr.
// 09/19/2002
// DO NOT DISTRIBUTE
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __LATCHRESTORE_H__
#define __LATCHRESTORE_H__

/*
	The purpose of the LatchRestore class is to allow you to override member variables for the scope
	of a function. Here's the code that this saves:

	void Foo::func(Team *overrideTeam)
	{
		Team *saveTeam = m_saveTeam;
		m_saveTeam = overrideTeam;

		// ... stuff	...

		if (fu)
		{
			// early return
			m_saveTeam = saveTeam;
			return true;
		}

		if (bar)
		{
			// early return
			m_saveTeam = saveTeam;
			return true;
		}

		if (munkees)
		{
			// early return
			m_saveTeam = saveTeam;
			return true;
		}


		m_saveTeam = saveTeam;
		return false;
	}

	Instead, the code would simply be:

	void Foo::func(Team *overrideTeam)
	{
		LatchRestore<Team *> latch(m_saveTeam, overrideTeam);

		// ... stuff	...

		if (fu)
			return true;

		if (bar)
			return true;

		if (munkees)
			return true;

		return false;
	}

*/

template <typename T> 
class LatchRestore
{
	protected:
		T valueToRestore;
		T& whereToRestore;

	public:
		LatchRestore(T& dest, const T& src) : whereToRestore(dest)
		{
			valueToRestore = dest;
			dest = src;
		}

		virtual ~LatchRestore()
		{
			whereToRestore = valueToRestore;
		}
};


#endif /* __LATCHRESTORE_H__ */

