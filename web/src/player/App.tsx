import { Route, Routes } from 'react-router'
import { HomePage } from './pages/HomePage'
import { DetailPage } from './pages/DetailPage'
import { PlayPage } from './pages/PlayPage'

export function App() {
  return (
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/content/:id" element={<DetailPage />} />
      <Route path="/play/:id" element={<PlayPage />} />
    </Routes>
  )
}
