import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import App from './App.tsx'
import { useLang } from './i18n/index.ts'

afterEach(cleanup)
beforeEach(() => useLang.getState().setLang('en'))

describe('App', () => {
  it('renders the pitwall heading', () => {
    render(<App />)
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('pitwall')
  })

  it('explains how to open the board when there is no token', () => {
    render(<App />)
    expect(screen.getByText(/No access token/)).toBeTruthy()
  })
})
